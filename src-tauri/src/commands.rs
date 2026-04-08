use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tauri::{AppHandle, State};

use tauri::Emitter;

use crate::audio::capture::{AudioCapture, AudioResampler};
use crate::audio::pipeline;
use crate::audio::{
    AudioMode, ModelInfo, ModelStatus, PipelineHandle, RecordResult, SttEngineResolvedPayload,
    WakeCommandInfo,
};
use crate::input::injector;
use crate::input::TextInputMethod;
use crate::system::detect::{self, SystemInfo};

const VAD_MODEL_FILENAME: &str = "silero_vad_v6.onnx";
const MEL_MODEL_FILENAME: &str = "melspectrogram.onnx";
const EMB_MODEL_FILENAME: &str = "embedding_model.onnx";
const WHISPER_DICTATION_MODEL_FILENAME: &str = "ggml-base.bin";
const PARAKEET_MODEL_DIR_NAME: &str = "parakeet-tdt";
const VOSK_MODEL_DIR_NAME: &str = "vosk-model-small-ru";
const WAKEWORDS_DIR_NAME: &str = "wakewords";
const SUBCOMMANDS_DIR_NAME: &str = "subcommands";

/// Platform-specific base data directory (ASCII-safe on Windows).
fn get_creo_data_dir() -> PathBuf {
    if cfg!(target_os = "windows") {
        PathBuf::from("C:\\creo-data")
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home).join(".local/share/creo")
    }
}

fn get_models_dir() -> PathBuf {
    get_creo_data_dir().join("models")
}

#[tauri::command]
pub fn check_models() -> ModelStatus {
    let dir = get_models_dir();
    let mel_path = dir.join(MEL_MODEL_FILENAME);
    let emb_path = dir.join(EMB_MODEL_FILENAME);
    let dictation_path = dir.join(WHISPER_DICTATION_MODEL_FILENAME);
    let vad_path = dir.join(VAD_MODEL_FILENAME);
    let vosk_path = dir.join(VOSK_MODEL_DIR_NAME);

    let models = vec![
        ModelInfo {
            name: "Silero VAD v6".to_string(),
            filename: VAD_MODEL_FILENAME.to_string(),
            path: vad_path.to_string_lossy().to_string(),
            exists: vad_path.exists(),
            size_hint: "~1.8 MB".to_string(),
            optional: false,
        },
        ModelInfo {
            name: "Mel Spectrogram (wake word)".to_string(),
            filename: MEL_MODEL_FILENAME.to_string(),
            path: mel_path.to_string_lossy().to_string(),
            exists: mel_path.exists(),
            size_hint: "~1 MB".to_string(),
            optional: false,
        },
        ModelInfo {
            name: "Speech Embedding (wake word)".to_string(),
            filename: EMB_MODEL_FILENAME.to_string(),
            path: emb_path.to_string_lossy().to_string(),
            exists: emb_path.exists(),
            size_hint: "~1.3 MB".to_string(),
            optional: false,
        },
        ModelInfo {
            name: "Whisper Base (dictation)".to_string(),
            filename: WHISPER_DICTATION_MODEL_FILENAME.to_string(),
            path: dictation_path.to_string_lossy().to_string(),
            exists: dictation_path.exists(),
            size_hint: "~150 MB".to_string(),
            optional: false,
        },
        ModelInfo {
            name: "Vosk Russian (subcommands)".to_string(),
            filename: VOSK_MODEL_DIR_NAME.to_string(),
            path: vosk_path.to_string_lossy().to_string(),
            exists: vosk_path.exists(),
            size_hint: "~45 MB (directory)".to_string(),
            optional: true,
        },
    ];

    let all_present = models.iter().filter(|m| !m.optional).all(|m| m.exists);

    ModelStatus {
        models_dir: dir.to_string_lossy().to_string(),
        all_present,
        models,
    }
}


fn start_pipeline_with_mode(
    app: AppHandle,
    state: &Arc<PipelineHandle>,
    initial_mode: AudioMode,
    stt_engine: &str,
) -> Result<(), String> {
    let dir = get_models_dir();
    let vad_path = dir.join(VAD_MODEL_FILENAME);
    let mel_path = dir.join(MEL_MODEL_FILENAME);
    let emb_path = dir.join(EMB_MODEL_FILENAME);

    let wakewords_dir = get_creo_data_dir().join(WAKEWORDS_DIR_NAME);
    let subcommands_dir = get_creo_data_dir().join(SUBCOMMANDS_DIR_NAME);
    let vosk_model_dir = dir.join(VOSK_MODEL_DIR_NAME);

    if !vad_path.exists() || !mel_path.exists() || !emb_path.exists() {
        return Err(format!(
            "Core models not found. Place them in: {}",
            dir.to_string_lossy()
        ));
    }

    let engine_type = crate::audio::stt::resolve_stt_engine(
        &dir,
        PARAKEET_MODEL_DIR_NAME,
        WHISPER_DICTATION_MODEL_FILENAME,
        stt_engine,
    )?;
    log::info!("STT engine: {} (preference: {})", engine_type, stt_engine);

    let _ = app.emit(
        "stt-engine-resolved",
        SttEngineResolvedPayload {
            engine: engine_type.to_string(),
        },
    );

    // Build a factory closure that creates the engine on the transcription thread.
    // This avoids Send issues — the engine is created where it will be used.
    let dictation_engine_factory: Box<
        dyn FnOnce() -> anyhow::Result<Box<dyn crate::audio::stt::DictationEngine>> + Send,
    > = match engine_type {
        "parakeet" => {
            let model_dir = dir
                .join(PARAKEET_MODEL_DIR_NAME)
                .to_string_lossy()
                .to_string();
            Box::new(move || {
                let engine = crate::audio::stt::ParakeetEngine::new(&model_dir)?;
                Ok(Box::new(engine) as Box<dyn crate::audio::stt::DictationEngine>)
            })
        }
        _ => {
            let model_path = dir
                .join(WHISPER_DICTATION_MODEL_FILENAME)
                .to_string_lossy()
                .to_string();
            if !PathBuf::from(&model_path).exists() {
                return Err(format!(
                    "Whisper dictation model not found: {}",
                    model_path
                ));
            }
            // TODO: language should come from settings
            Box::new(move || {
                let engine = crate::audio::stt::WhisperEngine::new(&model_path, "ru")?;
                Ok(Box::new(engine) as Box<dyn crate::audio::stt::DictationEngine>)
            })
        }
    };

    let vosk_model_path = if vosk_model_dir.exists() {
        Some(vosk_model_dir.to_string_lossy().to_string())
    } else {
        log::info!("Vosk model not found at {}, Tier 2 disabled", vosk_model_dir.display());
        None
    };

    pipeline::start_pipeline(
        app,
        state.clone(),
        initial_mode,
        vad_path.to_string_lossy().to_string(),
        mel_path.to_string_lossy().to_string(),
        emb_path.to_string_lossy().to_string(),
        wakewords_dir.to_string_lossy().to_string(),
        subcommands_dir.to_string_lossy().to_string(),
        vosk_model_path,
        dictation_engine_factory,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn start_listening(
    app: AppHandle,
    state: State<'_, Arc<PipelineHandle>>,
    stt_engine: Option<String>,
) -> Result<(), String> {
    if state.current_mode() != AudioMode::Off {
        return Ok(()); // Already running — idempotent no-op
    }
    let engine = stt_engine.as_deref().unwrap_or("auto");
    start_pipeline_with_mode(app, state.inner(), AudioMode::Standby, engine)
}

#[tauri::command]
pub fn start_dictation(
    app: AppHandle,
    state: State<'_, Arc<PipelineHandle>>,
    stt_engine: Option<String>,
) -> Result<(), String> {
    if state.current_mode() != AudioMode::Off {
        return Err("Pipeline already running".to_string());
    }
    let engine = stt_engine.as_deref().unwrap_or("auto");
    start_pipeline_with_mode(app, state.inner(), AudioMode::Dictation, engine)
}

/// Transition running pipeline to Dictation mode (no restart).
/// Use this when the pipeline is already running in Standby.
#[tauri::command]
pub fn transition_to_dictation(
    app: AppHandle,
    state: State<'_, Arc<PipelineHandle>>,
) -> Result<(), String> {
    let mode = state.current_mode();
    if mode == AudioMode::Off {
        return Err("Pipeline not running".to_string());
    }
    state.transition_mode(&app, AudioMode::Dictation);
    log::info!("Transitioned to Dictation (from {:?})", mode);
    Ok(())
}

/// Transition running pipeline back to Standby mode (no restart).
/// Use this to end dictation without killing the pipeline.
#[tauri::command]
pub fn transition_to_standby(
    app: AppHandle,
    state: State<'_, Arc<PipelineHandle>>,
) -> Result<(), String> {
    let mode = state.current_mode();
    if mode == AudioMode::Off {
        return Err("Pipeline not running".to_string());
    }
    state.transition_mode(&app, AudioMode::Standby);
    log::info!("Transitioned to Standby (from {:?})", mode);
    Ok(())
}

/// Get current pipeline mode (for frontend sync after reload).
#[tauri::command]
pub fn get_current_mode(
    state: State<'_, Arc<PipelineHandle>>,
) -> String {
    let mode = state.current_mode();
    serde_json::to_string(&mode).unwrap_or_else(|_| "\"off\"".to_string())
}

#[tauri::command]
pub fn stop_listening(
    app: AppHandle,
    state: State<'_, Arc<PipelineHandle>>,
) -> Result<(), String> {
    state.request_shutdown();
    state.join_threads()?;
    state.transition_mode(&app, AudioMode::Off);
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

/// Validate command name is safe for use as a filesystem directory name.
fn validate_command_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Command name cannot be empty".to_string());
    }
    if name.contains("..") || name.contains('/') || name.contains('\\') || name.contains('\0') {
        return Err(format!("Command name contains invalid characters: '{}'", name));
    }
    // Windows reserved characters
    if cfg!(target_os = "windows") && name.chars().any(|c| matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|')) {
        return Err(format!("Command name contains Windows-reserved characters: '{}'", name));
    }
    Ok(())
}

fn get_wakewords_dir() -> PathBuf {
    get_creo_data_dir().join("wakewords")
}

/// Record a voice sample using VAD: starts capturing, waits for speech,
/// records until silence detected, then extracts embeddings and saves.
/// Called from UI when user clicks "record" button during command creation.
/// Record a voice sample using VAD, extract embeddings, save.
/// `action` maps this command to a WakeAction: "await_subcommand", "start_dictation", "stop_dictation", "cancel_dictation".
#[tauri::command]
pub fn record_wake_sample(
    command_name: String,
    action: Option<String>,
    state: State<'_, Arc<PipelineHandle>>,
) -> Result<RecordResult, String> {
    validate_command_name(&command_name)?;

    use crate::audio::capture::capture_speech_vad;
    use crate::audio::wakeword::WakeWordDetector;
    use crate::audio::WakeAction;

    let dir = get_models_dir();
    let mel_path = dir.join(MEL_MODEL_FILENAME);
    let emb_path = dir.join(EMB_MODEL_FILENAME);
    let wakewords_dir = get_wakewords_dir();

    let vad_path = dir.join(VAD_MODEL_FILENAME);

    if !mel_path.exists() || !emb_path.exists() || !vad_path.exists() {
        return Err("Models not found".to_string());
    }

    let speech_buffer = capture_speech_vad(&vad_path, 5, 300, "Wake sample")?;

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
                .map_err(|_| format!("Unknown action: '{}'. Valid: await_subcommand, start_dictation, stop_dictation, cancel_dictation", action_str))?;
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

    // Signal running pipeline to reload wake word references
    state.request_reload_references()?;

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

/// Delete a wake word command: remove sample directory + clean config.json action mapping.
#[tauri::command]
pub fn delete_wake_command(
    command_name: String,
    state: State<'_, Arc<PipelineHandle>>,
) -> Result<(), String> {
    validate_command_name(&command_name)?;

    let wakewords_dir = get_wakewords_dir();
    let dir = wakewords_dir.join(&command_name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
        log::info!("Deleted wake command: {}", command_name);
    }

    // Clean up action mapping in config.json
    let config_path = wakewords_dir.join("config.json");
    if config_path.exists() {
        if let Ok(data) = std::fs::read_to_string(&config_path) {
            if let Ok(mut map) =
                serde_json::from_str::<std::collections::HashMap<String, serde_json::Value>>(&data)
            {
                if map.remove(&command_name).is_some() {
                    if let Ok(json) = serde_json::to_string_pretty(&map) {
                        let _ = std::fs::write(&config_path, json);
                    }
                    log::info!("Cleaned config.json entry: {}", command_name);
                }
            }
        }
    }

    // Signal running pipeline to reload wake word references
    state.request_reload_references()?;

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


// --- Subcommand management ---

fn get_subcommands_dir() -> PathBuf {
    get_creo_data_dir().join(SUBCOMMANDS_DIR_NAME)
}

/// Get the subcommand manifest (list of all defined subcommands).
#[tauri::command]
pub fn get_subcommands() -> crate::audio::subcommand::SubcommandManifest {
    use crate::audio::subcommand::SubcommandManifest;

    let dir = get_subcommands_dir();
    let path = dir.join("manifest.json");
    if !path.exists() {
        return SubcommandManifest::default();
    }
    match std::fs::read_to_string(&path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => SubcommandManifest::default(),
    }
}

/// Create a new subcommand definition in the manifest.
#[tauri::command]
pub fn create_subcommand(
    name: String,
    action: String,
    tier: String,
    state: State<'_, Arc<PipelineHandle>>,
) -> Result<(), String> {
    use crate::audio::subcommand::{SubcommandDef, SubcommandManifest, SubcommandTierKind};

    validate_command_name(&name)?;

    let tier_kind: SubcommandTierKind = serde_json::from_str(&format!("\"{}\"", tier))
        .map_err(|_| format!("Unknown tier: '{}'. Valid: dtw, vosk, llm", tier))?;

    let dir = get_subcommands_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let manifest_path = dir.join("manifest.json");
    let mut manifest: SubcommandManifest = if manifest_path.exists() {
        let data = std::fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?;
        serde_json::from_str(&data).unwrap_or_default()
    } else {
        SubcommandManifest::default()
    };

    // Check for duplicate
    if manifest.commands.iter().any(|c| c.name == name) {
        return Err(format!("Subcommand '{}' already exists", name));
    }

    manifest.commands.push(SubcommandDef {
        name: name.clone(),
        action,
        tier: tier_kind,
        phrases: Vec::new(),
        template: None,
    });

    let json = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    std::fs::write(&manifest_path, json).map_err(|e| e.to_string())?;

    // Create sample directory for DTW commands
    if tier_kind == SubcommandTierKind::Dtw {
        std::fs::create_dir_all(dir.join(&name)).map_err(|e| e.to_string())?;
    }

    state.request_reload_references()?;
    log::info!("Created subcommand: '{}'", name);
    Ok(())
}

/// Delete a subcommand definition and its samples.
#[tauri::command]
pub fn delete_subcommand(
    name: String,
    state: State<'_, Arc<PipelineHandle>>,
) -> Result<(), String> {
    use crate::audio::subcommand::SubcommandManifest;

    validate_command_name(&name)?;

    let dir = get_subcommands_dir();

    // Remove from manifest
    let manifest_path = dir.join("manifest.json");
    if manifest_path.exists() {
        let data = std::fs::read_to_string(&manifest_path).map_err(|e| e.to_string())?;
        let mut manifest: SubcommandManifest =
            serde_json::from_str(&data).unwrap_or_default();
        manifest.commands.retain(|c| c.name != name);
        let json = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
        std::fs::write(&manifest_path, json).map_err(|e| e.to_string())?;
    }

    // Remove sample directory
    let cmd_dir = dir.join(&name);
    if cmd_dir.exists() {
        std::fs::remove_dir_all(&cmd_dir).map_err(|e| e.to_string())?;
    }

    state.request_reload_references()?;
    log::info!("Deleted subcommand: '{}'", name);
    Ok(())
}

/// Record a DTW sample for a subcommand (same VAD-based flow as wake word recording).
#[tauri::command]
pub fn record_subcommand_sample(
    command_name: String,
    state: State<'_, Arc<PipelineHandle>>,
) -> Result<RecordResult, String> {
    use crate::audio::capture::capture_speech_vad;
    use crate::audio::embedding::save_frames_file;

    validate_command_name(&command_name)?;

    let dir = get_models_dir();
    let mel_path = dir.join(MEL_MODEL_FILENAME);
    let emb_path = dir.join(EMB_MODEL_FILENAME);
    let vad_path = dir.join(VAD_MODEL_FILENAME);
    let subcommands_dir = get_subcommands_dir();

    if !mel_path.exists() || !emb_path.exists() || !vad_path.exists() {
        return Err("Models not found".to_string());
    }

    let speech_buffer = capture_speech_vad(&vad_path, 5, 300, "Subcommand sample")?;

    // Extract embeddings and save
    let mut extractor = crate::audio::embedding::EmbeddingExtractor::new(
        &mel_path.to_string_lossy(),
        &emb_path.to_string_lossy(),
    )
    .map_err(|e| e.to_string())?;

    let frames = extractor
        .extract_frame_embeddings(&speech_buffer)
        .map_err(|e| e.to_string())?;
    if frames.is_empty() {
        return Err("Failed to extract embeddings from audio".to_string());
    }

    let cmd_dir = subcommands_dir.join(&command_name);
    std::fs::create_dir_all(&cmd_dir).map_err(|e| e.to_string())?;

    // Find next sample index
    let idx = std::fs::read_dir(&cmd_dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let stem = e.path().file_stem()?.to_str()?.to_string();
                    stem.strip_prefix("sample_")?.parse::<usize>().ok()
                })
                .max()
                .map(|max| max + 1)
                .unwrap_or(0)
        })
        .unwrap_or(0);

    let frames_path = cmd_dir.join(format!("sample_{}.frames", idx));
    save_frames_file(&frames_path, &frames).map_err(|e| e.to_string())?;

    let sample_count = std::fs::read_dir(&cmd_dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .and_then(|ext| ext.to_str())
                        == Some("frames")
                })
                .count()
        })
        .unwrap_or(0);

    log::info!(
        "Recorded subcommand sample '{}': {} frames, total {} samples",
        command_name,
        frames.len(),
        sample_count
    );

    state.request_reload_references()?;

    Ok(RecordResult {
        command_name,
        embedding_count: 1,
        total_samples: sample_count,
        path: frames_path.to_string_lossy().to_string(),
    })
}
