pub mod capture;
pub mod pipeline;
pub mod transcriber;
pub mod vad;

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WakeCommand {
    #[serde(rename = "прием")]
    Priem,
    #[serde(rename = "вписывай")]
    Vpisyvai,
    #[serde(rename = "готово")]
    Gotovo,
}

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
    pub shutdown: Arc<AtomicBool>,
    pub mode: Arc<AtomicU8>,
    pub threads: Mutex<Vec<JoinHandle<()>>>,
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

    pub fn set_mode(&self, mode: AudioMode) {
        self.mode.store(mode.as_u8(), Ordering::Relaxed);
    }

    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Relaxed)
    }

    pub fn request_shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}
