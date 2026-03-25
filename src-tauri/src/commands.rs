use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::audio::capture::{AudioCapture, AudioResampler};
use crate::audio::pipeline;
use crate::audio::{AudioMode, ModelInfo, ModelStatus, PipelineHandle};
use crate::input::injector;
use crate::input::TextInputMethod;
use crate::system::detect::{self, SystemInfo};

const VAD_MODEL_FILENAME: &str = "silero_vad_v6.onnx";
const MEL_MODEL_FILENAME: &str = "melspectrogram.onnx";
const EMB_MODEL_FILENAME: &str = "embedding_model.onnx";
const WHISPER_DICTATION_MODEL_FILENAME: &str = "ggml-base.bin";
const WAKEWORDS_DIR_NAME: &str = "wakewords";

/// Platform-specific models directory (ASCII-safe on Windows).
fn get_models_dir() -> PathBuf {
    if cfg!(target_os = "windows") {
        PathBuf::from("C:\\creo-data\\models")
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home).join(".local/share/creo/models")
    }
}

#[tauri::command]
pub fn check_models() -> ModelStatus {
    let dir = get_models_dir();
    let mel_path = dir.join(MEL_MODEL_FILENAME);
    let emb_path = dir.join(EMB_MODEL_FILENAME);
    let dictation_path = dir.join(WHISPER_DICTATION_MODEL_FILENAME);

    let vad_path = dir.join(VAD_MODEL_FILENAME);

    let models = vec![
        ModelInfo {
            name: "Silero VAD v5".to_string(),
            filename: VAD_MODEL_FILENAME.to_string(),
            path: vad_path.to_string_lossy().to_string(),
            exists: vad_path.exists(),
            size_hint: "~1.8 MB".to_string(),
        },
        ModelInfo {
            name: "Mel Spectrogram (wake word)".to_string(),
            filename: MEL_MODEL_FILENAME.to_string(),
            path: mel_path.to_string_lossy().to_string(),
            exists: mel_path.exists(),
            size_hint: "~1 MB".to_string(),
        },
        ModelInfo {
            name: "Speech Embedding (wake word)".to_string(),
            filename: EMB_MODEL_FILENAME.to_string(),
            path: emb_path.to_string_lossy().to_string(),
            exists: emb_path.exists(),
            size_hint: "~1.3 MB".to_string(),
        },
        ModelInfo {
            name: "Whisper Base (dictation)".to_string(),
            filename: WHISPER_DICTATION_MODEL_FILENAME.to_string(),
            path: dictation_path.to_string_lossy().to_string(),
            exists: dictation_path.exists(),
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

/// Shared pipeline startup logic: resolve model paths, validate, start pipeline in given mode.
fn start_pipeline_with_mode(
    app: AppHandle,
    state: &Arc<PipelineHandle>,
    initial_mode: AudioMode,
) -> Result<(), String> {
    let dir = get_models_dir();
    let vad_path = dir.join(VAD_MODEL_FILENAME);
    let mel_path = dir.join(MEL_MODEL_FILENAME);
    let emb_path = dir.join(EMB_MODEL_FILENAME);
    let dictation_path = dir.join(WHISPER_DICTATION_MODEL_FILENAME);

    let base_dir = if cfg!(target_os = "windows") {
        PathBuf::from("C:\\creo-data")
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home).join(".local/share/creo")
    };
    let wakewords_dir = base_dir.join(WAKEWORDS_DIR_NAME);

    if !vad_path.exists() || !mel_path.exists() || !emb_path.exists() || !dictation_path.exists() {
        return Err(format!(
            "Models not found. Place them in: {}",
            dir.to_string_lossy()
        ));
    }

    pipeline::start_pipeline(
        app,
        state.clone(),
        initial_mode,
        vad_path.to_string_lossy().to_string(),
        mel_path.to_string_lossy().to_string(),
        emb_path.to_string_lossy().to_string(),
        wakewords_dir.to_string_lossy().to_string(),
        dictation_path.to_string_lossy().to_string(),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn start_listening(
    app: AppHandle,
    state: State<'_, Arc<PipelineHandle>>,
) -> Result<(), String> {
    if state.current_mode() != AudioMode::Idle {
        return Err("Pipeline already running".to_string());
    }
    start_pipeline_with_mode(app, state.inner(), AudioMode::Listening)
}

#[tauri::command]
pub fn start_dictation(
    app: AppHandle,
    state: State<'_, Arc<PipelineHandle>>,
) -> Result<(), String> {
    if state.current_mode() != AudioMode::Idle {
        return Err("Pipeline already running".to_string());
    }
    start_pipeline_with_mode(app, state.inner(), AudioMode::Dictation)
}

#[tauri::command]
pub fn stop_listening(
    app: AppHandle,
    state: State<'_, Arc<PipelineHandle>>,
) -> Result<(), String> {
    state.request_shutdown();
    state.join_threads()?;
    state.transition_mode(&app, AudioMode::Idle);
    state.reset_shutdown();

    log::info!("Pipeline stopped");
    Ok(())
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

/// Inject text into the currently focused application.
#[tauri::command]
pub fn inject_text(text: String, method: Option<String>) -> Result<(), String> {
    let input_method = method
        .map(|m| TextInputMethod::from_str_lossy(&m))
        .unwrap_or(TextInputMethod::Paste);

    log::info!(
        "Injecting {} chars via {:?}",
        text.len(),
        input_method
    );

    injector::inject_text(&text, input_method).map_err(|e| e.to_string())
}

/// Detect system hardware: GPU, CPU, RAM, display server, OS.
#[tauri::command]
pub fn detect_system() -> SystemInfo {
    detect::detect_system()
}

// --- Wake word management ---

fn get_wakewords_dir() -> PathBuf {
    if cfg!(target_os = "windows") {
        PathBuf::from("C:\\creo-data\\wakewords")
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home).join(".local/share/creo/wakewords")
    }
}

/// Record a voice sample using VAD: starts capturing, waits for speech,
/// records until silence detected, then extracts embeddings and saves.
/// Called from UI when user clicks "record" button during command creation.
/// Record a voice sample using VAD, extract embeddings, save.
/// `action` maps this command to a WakeAction: "command_mode", "start_dictation", "stop_dictation".
#[tauri::command]
pub fn record_wake_sample(command_name: String, action: Option<String>) -> Result<RecordResult, String> {
    use crate::audio::wakeword::WakeWordDetector;
    use crate::audio::vad::SileroVad;
    use crate::audio::WakeAction;

    let dir = get_models_dir();
    let mel_path = dir.join(MEL_MODEL_FILENAME);
    let emb_path = dir.join(EMB_MODEL_FILENAME);
    let wakewords_dir = get_wakewords_dir();

    let vad_path = dir.join(VAD_MODEL_FILENAME);

    if !mel_path.exists() || !emb_path.exists() || !vad_path.exists() {
        return Err("Models not found".to_string());
    }

    // Start capture + VAD
    let (capture, rx, sample_rate) = AudioCapture::start().map_err(|e| e.to_string())?;
    let mut resampler = AudioResampler::new(sample_rate).map_err(|e| e.to_string())?;
    let mut vad = SileroVad::new(&vad_path.to_string_lossy()).map_err(|e| e.to_string())?;

    let chunk_size = SileroVad::chunk_size();
    let mut vad_buffer: Vec<f32> = Vec::new();
    let mut speech_buffer: Vec<f32> = Vec::new();
    let mut is_speaking = false;
    let mut silence_start: Option<Instant> = None;
    let mut speech_detected = false;
    let timeout = Instant::now();

    // Wait up to 5 seconds for speech, then record until 300ms silence
    while timeout.elapsed() < Duration::from_secs(5) {
        if let Ok(chunk) = rx.recv_timeout(Duration::from_millis(50)) {
            let resampled = resampler.process(&chunk);
            vad_buffer.extend_from_slice(&resampled);

            while vad_buffer.len() >= chunk_size {
                let vad_chunk: Vec<f32> = vad_buffer.drain(..chunk_size).collect();

                let is_speech = vad
                    .is_speech(&vad_chunk)
                    .unwrap_or(false);

                if is_speech {
                    silence_start = None;
                    if !is_speaking {
                        is_speaking = true;
                        speech_detected = true;
                        log::info!("Wake sample: speech started");
                    }
                    speech_buffer.extend_from_slice(&vad_chunk);
                } else if is_speaking {
                    speech_buffer.extend_from_slice(&vad_chunk);
                    if silence_start.is_none() {
                        silence_start = Some(Instant::now());
                    }
                    if let Some(start) = silence_start {
                        if start.elapsed() > Duration::from_millis(300) {
                            log::info!("Wake sample: speech ended, {} samples", speech_buffer.len());
                            break;
                        }
                    }
                }
            }

            // Break outer loop if speech ended
            if speech_detected && !is_speaking {
                break;
            }
            if is_speaking && silence_start.map(|s| s.elapsed() > Duration::from_millis(300)).unwrap_or(false) {
                break;
            }
        }
    }

    drop(capture);

    if speech_buffer.is_empty() {
        return Err("No speech detected. Please try again.".to_string());
    }

    // Extract mean embedding and save
    let mut detector = WakeWordDetector::new(
        &mel_path.to_string_lossy(),
        &emb_path.to_string_lossy(),
        &wakewords_dir.to_string_lossy(),
    )
    .map_err(|e| e.to_string())?;

    log::info!(
        "Wake sample: speech_buffer {} samples ({:.1}ms)",
        speech_buffer.len(),
        speech_buffer.len() as f64 / 16.0
    );

    let path = detector
        .save_reference(&command_name, &speech_buffer)
        .map_err(|e| e.to_string())?;

    // Save action mapping if provided
    if let Some(action_str) = action {
        let wake_action: WakeAction =
            serde_json::from_str(&format!("\"{}\"", action_str))
                .map_err(|_| format!("Unknown action: '{}'. Valid: command_mode, start_dictation, stop_dictation", action_str))?;
        detector
            .save_action_mapping(&command_name, wake_action)
            .map_err(|e| e.to_string())?;
    }

    let sample_count = get_sample_count(&command_name);

    log::info!(
        "Recorded wake sample '{}': total {} samples",
        command_name,
        sample_count
    );

    Ok(RecordResult {
        command_name,
        embedding_count: 1, // one mean embedding per sample
        total_samples: sample_count,
        path: path.to_string_lossy().to_string(),
    })
}

/// Get list of all trained wake word commands with sample counts.
#[tauri::command]
pub fn get_wake_commands() -> Vec<WakeCommandInfo> {
    let dir = get_wakewords_dir();
    if !dir.exists() {
        return Vec::new();
    }

    let mut commands = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let name = entry.file_name().to_string_lossy().to_string();
                let count = get_sample_count(&name);
                commands.push(WakeCommandInfo {
                    name,
                    sample_count: count,
                });
            }
        }
    }

    commands
}

/// Delete a wake word command and all its samples.
#[tauri::command]
pub fn delete_wake_command(command_name: String) -> Result<(), String> {
    let dir = get_wakewords_dir().join(&command_name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
        log::info!("Deleted wake command: {}", command_name);
    }
    Ok(())
}

fn get_sample_count(command_name: &str) -> usize {
    let dir = get_wakewords_dir().join(command_name);
    std::fs::read_dir(&dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .and_then(|ext| ext.to_str())
                        == Some("emb")
                })
                .count()
        })
        .unwrap_or(0)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordResult {
    pub command_name: String,
    pub embedding_count: usize,
    pub total_samples: usize,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WakeCommandInfo {
    pub name: String,
    pub sample_count: usize,
}
