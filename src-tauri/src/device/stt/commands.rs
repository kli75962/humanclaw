use std::sync::Mutex;
use std::sync::atomic::Ordering;
use tauri::AppHandle;

#[cfg(not(target_os = "android"))]
use serde::Serialize;
#[cfg(not(target_os = "android"))]
use tauri::Emitter;

use super::capture::CaptureHandle;
use super::transcribe;
#[cfg(not(target_os = "android"))]
use super::capture;
#[cfg(not(target_os = "android"))]
use super::types::resolve_google_api_key;

struct SttSession {
    handle: CaptureHandle,
    api_key: String,
    primary_language: String,
    alternative_languages: Vec<String>,
}

static CAPTURE: Mutex<Option<SttSession>> = Mutex::new(None);

#[cfg(not(target_os = "android"))]
#[derive(Serialize, Clone)]
struct SttPartialPayload {
    text: String,
}

/// Start native microphone capture and stream partial transcriptions every 5s.
/// `api_key` overrides GOOGLE_API_KEY from .secrets.
/// The cpal::Stream is !Send, so it lives inside a dedicated OS thread.
#[tauri::command]
pub fn stt_start(
    app: AppHandle,
    api_key: Option<String>,
    languages: Option<Vec<String>>,
) -> Result<(), String> {
    #[cfg(not(target_os = "android"))]
    {
        use std::sync::mpsc;

        let mut guard = CAPTURE.lock().map_err(|e| e.to_string())?;
        if guard.is_some() {
            return Ok(()); // already recording
        }

        let key = resolve_google_api_key(api_key.as_deref())?;
        let (primary_language, alternative_languages) = parse_stt_languages(languages);
        let (tx, rx) = mpsc::channel::<Result<CaptureHandle, String>>();

        std::thread::spawn(move || {
            match capture::start_capture() {
                Ok((handle, stream)) => {
                    let recording = handle.recording.clone();
                    let _ = tx.send(Ok(handle));
                    while recording.load(Ordering::Relaxed) {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                    drop(stream);
                }
                Err(e) => {
                    let _ = tx.send(Err(e));
                }
            }
        });

        let handle = rx
            .recv()
            .map_err(|_| "Capture thread failed to start".to_string())??
            ;

        // Clone Arc refs for the periodic streaming task before moving handle into state.
        let samples_arc = handle.samples.clone();
        let recording_arc = handle.recording.clone();
        let sample_rate = handle.sample_rate;
        let key_clone = key.clone();
        let primary_language_clone = primary_language.clone();
        let alternative_languages_clone = alternative_languages.clone();
        let app_clone = app.clone();

        // Spawn a Tokio task that sends only the NEW audio captured since the last
        // chunk to Google STT every 3s. Each API call is short (~3s of audio) so
        // it returns quickly, giving a near-streaming feel.
        tauri::async_runtime::spawn(async move {
            let mut last_sent: usize = 0;
            let mut running_transcript = String::new();
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                if !recording_arc.load(Ordering::Relaxed) {
                    break;
                }
                let chunk: Vec<f32> = {
                    let Ok(buf) = samples_arc.lock() else { break };
                    let new_samples = &buf[last_sent..];
                    if new_samples.is_empty() {
                        continue;
                    }
                    let chunk = new_samples.to_vec();
                    last_sent = buf.len();
                    chunk
                };
                if let Ok(text) = transcribe::transcribe(
                    &chunk,
                    sample_rate,
                    &key_clone,
                    &primary_language_clone,
                    &alternative_languages_clone,
                ).await {
                    if !text.is_empty() {
                        if !running_transcript.is_empty() {
                            running_transcript.push(' ');
                        }
                        running_transcript.push_str(&text);
                        let _ = app_clone.emit("stt-partial", SttPartialPayload {
                            text: running_transcript.clone(),
                        });
                    }
                }
            }
        });

        *guard = Some(SttSession {
            handle,
            api_key: key,
            primary_language,
            alternative_languages,
        });
        Ok(())
    }
    #[cfg(target_os = "android")]
    {
        let _ = (app, api_key, languages);
        Err("Native capture not available on Android".to_string())
    }
}

/// Stop capture and return the final transcript of the complete recording.
#[tauri::command]
pub async fn stt_stop() -> Result<String, String> {
    let (samples, sample_rate, api_key, primary_language, alternative_languages) = {
        let mut guard = CAPTURE.lock().map_err(|e| e.to_string())?;
        let session = guard.take().ok_or("Not recording")?;

        session.handle.recording.store(false, Ordering::Relaxed);

        let samples = {
            let mut buf = session.handle.samples.lock().map_err(|e| e.to_string())?;
            std::mem::take(&mut *buf)
        };

        (
            samples,
            session.handle.sample_rate,
            session.api_key,
            session.primary_language,
            session.alternative_languages,
        )
    };

    if samples.is_empty() {
        return Ok(String::new());
    }

    transcribe::transcribe(
        &samples,
        sample_rate,
        &api_key,
        &primary_language,
        &alternative_languages,
    ).await
}

#[cfg(not(target_os = "android"))]
fn parse_stt_languages(input: Option<Vec<String>>) -> (String, Vec<String>) {
    let fallback = vec![
        "en-US".to_string(),
        "yue-Hant-HK".to_string(),
        "cmn-Hans-CN".to_string(),
    ];

    let mut langs = input
        .unwrap_or(fallback)
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();

    if langs.is_empty() {
        langs = vec![
            "en-US".to_string(),
            "yue-Hant-HK".to_string(),
            "cmn-Hans-CN".to_string(),
        ];
    }

    let primary = langs.remove(0);
    let alternatives = langs.into_iter().take(3).collect::<Vec<_>>();
    (primary, alternatives)
}

/// Android-only: run one-shot native speech recognition and return transcript text.
#[tauri::command]
pub async fn stt_android_once(app: AppHandle) -> Result<String, String> {
    #[cfg(target_os = "android")]
    {
        use serde::Deserialize;
        use serde_json::json;
        use tauri::Manager;
        use crate::device::phone::plugin::PhoneControlHandle;

        #[derive(Deserialize)]
        struct Resp {
            text: String,
        }

        let handle = app.state::<PhoneControlHandle<tauri::Wry>>();
        let resp = handle
            .0
            .run_mobile_plugin::<Resp>("recognizeSpeech", json!({}))
            .map_err(|e| e.to_string())?;
        Ok(resp.text)
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Err("stt_android_once is only available on Android".to_string())
    }
}

/// Android-only: cancel active native speech recognition session.
#[tauri::command]
pub async fn stt_android_cancel(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        use serde::Deserialize;
        use serde_json::json;
        use tauri::Manager;
        use crate::device::phone::plugin::PhoneControlHandle;

        #[derive(Deserialize)]
        struct Resp {}

        let handle = app.state::<PhoneControlHandle<tauri::Wry>>();
        handle
            .0
            .run_mobile_plugin::<Resp>("cancelSpeechRecognition", json!({}))
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Err("stt_android_cancel is only available on Android".to_string())
    }
}
