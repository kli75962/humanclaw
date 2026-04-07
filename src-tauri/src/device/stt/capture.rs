use std::sync::{Arc, Mutex, atomic::AtomicBool};

#[cfg(not(target_os = "android"))]
use std::sync::atomic::Ordering;

#[cfg(not(target_os = "android"))]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// Shared audio buffer + control flags — safe to put in a static Mutex.
pub struct CaptureHandle {
    pub samples: Arc<Mutex<Vec<f32>>>,
    pub sample_rate: u32,
    pub recording: Arc<AtomicBool>,
}

/// Start capturing audio from the default input device.
/// Returns a `CaptureHandle` (Send+Sync) for the shared buffer, plus the
/// `cpal::Stream` separately (it is `!Send` and must stay on this thread).
#[cfg(not(target_os = "android"))]
pub fn start_capture() -> Result<(CaptureHandle, cpal::Stream), String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or("No input device available")?;

    let config = device
        .default_input_config()
        .map_err(|e| format!("No input config: {e}"))?;

    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;

    let samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let recording = Arc::new(AtomicBool::new(true));

    let samples_w = samples.clone();
    let recording_r = recording.clone();

    let err_fn = |e: cpal::StreamError| eprintln!("cpal stream error: {e}");

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if !recording_r.load(Ordering::Relaxed) { return; }
                let mono: Vec<f32> = if channels == 1 {
                    data.to_vec()
                } else {
                    data.chunks(channels).map(|c| c[0]).collect()
                };
                if let Ok(mut buf) = samples_w.lock() {
                    buf.extend_from_slice(&mono);
                }
            },
            err_fn,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.into(),
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                if !recording_r.load(Ordering::Relaxed) { return; }
                let mono: Vec<f32> = if channels == 1 {
                    data.iter().map(|&s| s as f32 / 32768.0).collect()
                } else {
                    data.chunks(channels).map(|c| c[0] as f32 / 32768.0).collect()
                };
                if let Ok(mut buf) = samples_w.lock() {
                    buf.extend_from_slice(&mono);
                }
            },
            err_fn,
            None,
        ),
        other => return Err(format!("Unsupported sample format: {other:?}")),
    }
    .map_err(|e| format!("Failed to build stream: {e}"))?;

    stream.play().map_err(|e| format!("Failed to start stream: {e}"))?;

    let handle = CaptureHandle {
        samples,
        sample_rate,
        recording,
    };

    Ok((handle, stream))
}
