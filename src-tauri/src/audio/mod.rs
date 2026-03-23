pub mod capture;
pub mod pipeline;
pub mod transcriber;
pub mod vad;
pub mod wakeword;

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use tauri::{AppHandle, Emitter};

/// Mirrors frontend AudioMode enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioMode {
    Idle,
    Listening,
    Dictation,
    Processing,
}

impl AudioMode {
    pub fn as_u8(self) -> u8 {
        match self {
            AudioMode::Idle => 0,
            AudioMode::Listening => 1,
            AudioMode::Dictation => 2,
            AudioMode::Processing => 3,
        }
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => AudioMode::Listening,
            2 => AudioMode::Dictation,
            3 => AudioMode::Processing,
            _ => AudioMode::Idle,
        }
    }
}

/// Actions that wake commands can trigger.
/// Names are declarative English — user-facing command names can be in any language.
/// Mapping from user name → action is stored in wakewords/config.json.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WakeAction {
    CommandMode,
    StartDictation,
    StopDictation,
}

/// Legacy alias for backward compatibility in pipeline code.
/// TODO: rename all usages to WakeAction after full migration.
pub type WakeCommand = WakeAction;

// --- Event payloads (Rust → Frontend) ---

#[derive(Debug, Clone, Serialize)]
pub struct AudioStatePayload {
    pub mode: AudioMode,
}

#[derive(Debug, Clone, Serialize)]
pub struct WakeCommandPayload {
    pub command: WakeCommand,
}

#[derive(Debug, Clone, Serialize)]
pub struct TranscriptionPayload {
    pub text: String,
    #[serde(rename = "isFinal")]
    pub is_final: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct VadStatePayload {
    #[serde(rename = "isSpeech")]
    pub is_speech: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorPayload {
    pub message: String,
}

// --- Model info (for check_models command) ---

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

// --- Channel messages between threads ---

pub enum TranscriptionRequest {
    WakeWordCheck(Vec<f32>),
    DictationChunk(Vec<f32>),
    Shutdown,
}

pub enum TranscriptionResult {
    WakeCommand(WakeCommand),
    DictationText { text: String, is_final: bool },
    NoMatch,
}

// --- Pipeline handle (Tauri managed state) ---

pub struct PipelineHandle {
    shutdown: Arc<AtomicBool>,
    mode: Arc<AtomicU8>,
    threads: Mutex<Vec<JoinHandle<()>>>,
}

impl PipelineHandle {
    pub fn new() -> Self {
        Self {
            shutdown: Arc::new(AtomicBool::new(false)),
            mode: Arc::new(AtomicU8::new(AudioMode::Idle.as_u8())),
            threads: Mutex::new(Vec::new()),
        }
    }

    pub fn current_mode(&self) -> AudioMode {
        AudioMode::from_u8(self.mode.load(Ordering::Relaxed))
    }

    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Relaxed)
    }

    pub fn request_shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }

    pub fn reset_shutdown(&self) {
        self.shutdown.store(false, Ordering::Relaxed);
    }

    /// Set mode AND emit event to frontend. Use this for all mode transitions
    /// to prevent state desync between Rust and frontend.
    pub fn transition_mode(&self, app: &AppHandle, new_mode: AudioMode) {
        self.mode.store(new_mode.as_u8(), Ordering::Relaxed);
        let _ = app.emit("audio-state-changed", AudioStatePayload { mode: new_mode });
    }

    pub fn join_threads(&self) -> Result<(), String> {
        let mut threads = self.threads.lock().map_err(|e| e.to_string())?;
        for handle in threads.drain(..) {
            let _ = handle.join();
        }
        Ok(())
    }

    pub fn push_thread(&self, handle: JoinHandle<()>) -> Result<(), String> {
        let mut threads = self.threads.lock().map_err(|e| e.to_string())?;
        threads.push(handle);
        Ok(())
    }
}
