use base64::Engine as _;
use serde_json::Value;

const GOOGLE_STT_URL: &str = "https://speech.googleapis.com/v1/speech:recognize";

/// Convert f32 samples (-1.0..1.0) to i16 LE PCM bytes.
fn to_pcm(samples: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for &s in samples {
        let s = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

/// Send accumulated audio to Google Cloud Speech-to-Text v1 and return the transcript.
/// `api_key` must be a valid Google Cloud API key with Speech-to-Text API enabled.
pub async fn transcribe(
    samples: &[f32],
    sample_rate: u32,
    api_key: &str,
    primary_language: &str,
    alternative_languages: &[String],
) -> Result<String, String> {
    if samples.is_empty() {
        return Ok(String::new());
    }

    let pcm = to_pcm(samples);
    let audio_b64 = base64::engine::general_purpose::STANDARD.encode(&pcm);

    let mut config = serde_json::Map::new();
    config.insert("encoding".into(), serde_json::json!("LINEAR16"));
    config.insert("sampleRateHertz".into(), serde_json::json!(sample_rate));
    config.insert("languageCode".into(), serde_json::json!(primary_language));
    config.insert("enableAutomaticPunctuation".into(), serde_json::json!(true));
    if !alternative_languages.is_empty() {
        config.insert(
            "alternativeLanguageCodes".into(),
            serde_json::json!(alternative_languages),
        );
    }

    let body = serde_json::json!({
        "config": config,
        "audio": {
            "content": audio_b64,
        }
    });

    let url = format!("{GOOGLE_STT_URL}?key={api_key}");

    let response = super::stt_client()
        .post(&url)
        .json(&body)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| format!("Google STT unreachable: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Google STT error {status}: {text}"));
    }

    let json: Value = response
        .json()
        .await
        .map_err(|e| format!("Invalid Google STT response: {e}"))?;

    // {"results": [{"alternatives": [{"transcript": "...", "confidence": 0.97}]}]}
    let mut transcript = String::new();
    if let Some(results) = json["results"].as_array() {
        for result in results {
            if let Some(t) = result["alternatives"][0]["transcript"].as_str() {
                if !transcript.is_empty() {
                    transcript.push(' ');
                }
                transcript.push_str(t);
            }
        }
    }

    Ok(transcript.trim().to_string())
}
