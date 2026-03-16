use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::audio::capture::{AudioCapture, AudioResampler};
use crate::audio::pipeline;
use crate::audio::{AudioMode, AudioStatePayload, PipelineHandle};

const VAD_MODEL_FILENAME: &str = "silero_vad_v5.onnx";
const WHISPER_MODEL_FILENAME: &str = "ggml-base.bin";

/// Platform-specific models directory (ASCII-safe on Windows).
fn get_models_dir() -> PathBuf {
    if cfg!(target_os = "windows") {
        PathBuf::from("C:\\creo-data\\models")
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home).join(".local/share/creo/models")
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub name: String,
    pub filename: String,
    pub path: String,
    pub exists: bool,
    #[serde(rename = "sizeHint")]
    pub size_hint: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelStatus {
    #[serde(rename = "modelsDir")]
    pub models_dir: String,
    #[serde(rename = "allPresent")]
    pub all_present: bool,
    pub models: Vec<ModelInfo>,
}

#[tauri::command]
pub fn check_models() -> ModelStatus {
    let dir = get_models_dir();
    let vad_path = dir.join(VAD_MODEL_FILENAME);
    let whisper_path = dir.join(WHISPER_MODEL_FILENAME);

    let models = vec![
        ModelInfo {
            name: "Silero VAD v5".to_string(),
            filename: VAD_MODEL_FILENAME.to_string(),
            path: vad_path.to_string_lossy().to_string(),
            exists: vad_path.exists(),
            size_hint: "~1.8 MB".to_string(),
        },
        ModelInfo {
            name: "Whisper Base (GGML)".to_string(),
            filename: WHISPER_MODEL_FILENAME.to_string(),
            path: whisper_path.to_string_lossy().to_string(),
            exists: whisper_path.exists(),
            size_hint: "~150 MB".to_string(),
        },
    ];

    let all_present = models.iter().all(|m| m.exists);

    ModelStatus {
        models_dir: dir.to_string_lossy().to_string(),
        all_present,
        models,
    }
}

#[tauri::command]
pub fn start_listening(
    app: AppHandle,
    state: State<'_, Arc<PipelineHandle>>,
) -> Result<(), String> {
    if state.current_mode() != AudioMode::Idle {
        return Err("Pipeline already running".to_string());
    }

    let dir = get_models_dir();
    let vad_path = dir.join(VAD_MODEL_FILENAME);
    let whisper_path = dir.join(WHISPER_MODEL_FILENAME);

    if !vad_path.exists() || !whisper_path.exists() {
        return Err(format!(
            "Models not found. Place them in: {}",
            dir.to_string_lossy()
        ));
    }

    pipeline::start_pipeline(
        app,
        state.inner().clone(),
        vad_path.to_string_lossy().to_string(),
        whisper_path.to_string_lossy().to_string(),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn stop_listening(
    app: AppHandle,
    state: State<'_, Arc<PipelineHandle>>,
) -> Result<(), String> {
    state.request_shutdown();

    let mut threads = state.threads.lock().map_err(|e| e.to_string())?;
    for handle in threads.drain(..) {
        let _ = handle.join();
    }

    state.set_mode(AudioMode::Idle);
    state
        .shutdown
        .store(false, std::sync::atomic::Ordering::Relaxed);

    let _ = app.emit(
        "audio-state-changed",
        AudioStatePayload {
            mode: AudioMode::Idle,
        },
    );

    log::info!("Pipeline stopped");
    Ok(())
}

#[tauri::command]
pub fn get_audio_state(state: State<'_, Arc<PipelineHandle>>) -> AudioMode {
    state.current_mode()
}

/// Test command: capture 3 seconds of audio, log RMS levels.
#[tauri::command]
pub fn test_capture() -> Result<String, String> {
    let (capture, rx, sample_rate) = AudioCapture::start().map_err(|e| e.to_string())?;
    let mut resampler = AudioResampler::new(sample_rate).map_err(|e| e.to_string())?;

    let start = Instant::now();
    let mut total_samples = 0u64;
    let mut rms_sum = 0.0f64;
    let mut rms_count = 0u64;

    while start.elapsed() < Duration::from_secs(3) {
        if let Ok(chunk) = rx.recv_timeout(Duration::from_millis(100)) {
            let resampled = resampler.process(&chunk);
            total_samples += resampled.len() as u64;

            if !resampled.is_empty() {
                let rms = (resampled
                    .iter()
                    .map(|&s| (s * s) as f64)
                    .sum::<f64>()
                    / resampled.len() as f64)
                    .sqrt();
                rms_sum += rms;
                rms_count += 1;
            }
        }
    }

    drop(capture);

    let avg_rms = if rms_count > 0 {
        rms_sum / rms_count as f64
    } else {
        0.0
    };
    let result = format!(
        "Captured {} samples (resampled 16kHz)\nDuration: 3s\nAvg RMS: {:.6}\nNative rate: {}Hz",
        total_samples, avg_rms, sample_rate
    );

    log::info!("{}", result);
    Ok(result)
}
