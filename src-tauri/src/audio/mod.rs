pub mod capture;
pub mod embedding;
pub mod pipeline;
pub mod stt;
pub mod subcommand;
pub mod transcriber;
pub mod vad;
pub mod wakeword;

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use tauri::{AppHandle, Emitter};

/// Mirrors frontend AudioMode enum.
/// Wire format is pinned via explicit `#[serde(rename)]` — do NOT use `rename_all`.
/// Renaming a variant does not change the serialized value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioMode {
    #[serde(rename = "off")]
    Off,
    #[serde(rename = "standby")]
    Standby,
    #[serde(rename = "dictation")]
    Dictation,
    #[serde(rename = "processing")]
    Processing,
    #[serde(rename = "awaiting_subcommand")]
    AwaitingSubcommand,
}

impl AudioMode {
    pub fn as_u8(self) -> u8 {
        match self {
            AudioMode::Off => 0,
            AudioMode::Standby => 1,
            AudioMode::Dictation => 2,
            AudioMode::Processing => 3,
            AudioMode::AwaitingSubcommand => 4,
        }
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => AudioMode::Standby,
            2 => AudioMode::Dictation,
            3 => AudioMode::Processing,
            4 => AudioMode::AwaitingSubcommand,
            _ => AudioMode::Off,
        }
    }
}

/// Actions that wake commands can trigger.
/// Names are declarative English — user-facing command names can be in any language.
/// Mapping from user name → action is stored in wakewords/config.json.
/// Wire format is pinned via explicit `#[serde(rename)]` — do NOT use `rename_all`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WakeAction {
    #[serde(rename = "await_subcommand", alias = "command_mode")]
    AwaitSubcommand,
    #[serde(rename = "start_dictation")]
    StartDictation,
    #[serde(rename = "stop_dictation")]
    StopDictation,
    #[serde(rename = "cancel_dictation")]
    CancelDictation,
}

// WakeCommand alias removed — all usages migrated to WakeAction.

// --- Event payloads (Rust → Frontend) ---

#[derive(Debug, Clone, Serialize)]
pub struct AudioStatePayload {
    pub mode: AudioMode,
}

#[derive(Debug, Clone, Serialize)]
pub struct WakeActionPayload {
    pub command: WakeAction,
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

#[derive(Debug, Clone, Serialize)]
pub struct SttEngineResolvedPayload {
    pub engine: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubcommandMatchPayload {
    pub command: String,
    pub action: String,
    pub confidence: f32,
    pub tier: u8,
    pub params: std::collections::HashMap<String, String>,
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
    SubcommandCheck(Vec<f32>),
    DictationChunk(Vec<f32>),
    ReloadReferences,
    Shutdown,
}

pub enum TranscriptionResult {
    WakeAction(WakeAction),
    SubcommandMatch(subcommand::SubcommandMatch),
    DictationText { text: String, is_final: bool },
    NoMatch,
}

// --- Pipeline handle (Tauri managed state) ---

pub struct PipelineHandle {
    shutdown: Arc<AtomicBool>,
    mode: Arc<AtomicU8>,
    /// Mode before entering Processing — used to return on NoMatch.
    pre_processing_mode: Arc<AtomicU8>,
    threads: Mutex<Vec<JoinHandle<()>>>,
    /// Channel to send signals to the transcription thread (reload references, etc.).
    trans_tx: Mutex<Option<crossbeam_channel::Sender<TranscriptionRequest>>>,
}

impl PipelineHandle {
    pub fn new() -> Self {
        Self {
            shutdown: Arc::new(AtomicBool::new(false)),
            mode: Arc::new(AtomicU8::new(AudioMode::Off.as_u8())),
            pre_processing_mode: Arc::new(AtomicU8::new(AudioMode::Off.as_u8())),
            threads: Mutex::new(Vec::new()),
            trans_tx: Mutex::new(None),
        }
    }

    /// Store a reference to the transcription channel sender (set during pipeline start).
    pub fn set_trans_tx(&self, tx: crossbeam_channel::Sender<TranscriptionRequest>) {
        if let Ok(mut guard) = self.trans_tx.lock() {
            *guard = Some(tx);
        }
    }

    /// Send a signal to the transcription thread to reload wake word references.
    pub fn request_reload_references(&self) {
        if let Ok(guard) = self.trans_tx.lock() {
            if let Some(tx) = guard.as_ref() {
                let _ = tx.try_send(TranscriptionRequest::ReloadReferences);
            }
        }
    }

    pub fn current_mode(&self) -> AudioMode {
        AudioMode::from_u8(self.mode.load(Ordering::SeqCst))
    }

    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::SeqCst)
    }

    pub fn request_shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }

    pub fn reset_shutdown(&self) {
        self.shutdown.store(false, Ordering::SeqCst);
    }

    /// Save current mode before entering Processing, so NoMatch can restore it.
    pub fn save_pre_processing_mode(&self) {
        let current = self.mode.load(Ordering::SeqCst);
        self.pre_processing_mode.store(current, Ordering::SeqCst);
    }

    pub fn pre_processing_mode(&self) -> AudioMode {
        AudioMode::from_u8(self.pre_processing_mode.load(Ordering::SeqCst))
    }

    /// Set mode AND emit event to frontend. Use this for all mode transitions
    /// to prevent state desync between Rust and frontend.
    pub fn transition_mode(&self, app: &AppHandle, new_mode: AudioMode) {
        self.mode.store(new_mode.as_u8(), Ordering::SeqCst);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Pinned wire format for AudioMode — if this fails, you changed a serialized value.
    /// Update the test ONLY if the change is intentional AND you've handled migration
    /// of existing data (Tauri Store, events, frontend enum values).
    #[test]
    fn audio_mode_serialization_stability() {
        assert_eq!(serde_json::to_string(&AudioMode::Off).unwrap(), "\"off\"");
        assert_eq!(serde_json::to_string(&AudioMode::Standby).unwrap(), "\"standby\"");
        assert_eq!(serde_json::to_string(&AudioMode::Dictation).unwrap(), "\"dictation\"");
        assert_eq!(serde_json::to_string(&AudioMode::Processing).unwrap(), "\"processing\"");
        assert_eq!(
            serde_json::to_string(&AudioMode::AwaitingSubcommand).unwrap(),
            "\"awaiting_subcommand\""
        );
    }

    /// Pinned wire format for WakeAction — persisted in wakewords/config.json.
    /// Breaking this silently disables all wake word detection.
    #[test]
    fn wake_action_serialization_stability() {
        assert_eq!(
            serde_json::to_string(&WakeAction::AwaitSubcommand).unwrap(),
            "\"await_subcommand\""
        );
        assert_eq!(serde_json::to_string(&WakeAction::StartDictation).unwrap(), "\"start_dictation\"");
        assert_eq!(serde_json::to_string(&WakeAction::StopDictation).unwrap(), "\"stop_dictation\"");
        assert_eq!(
            serde_json::to_string(&WakeAction::CancelDictation).unwrap(),
            "\"cancel_dictation\""
        );
    }

    /// Backward compatibility: old "command_mode" config values must deserialize.
    #[test]
    fn wake_action_backward_compat() {
        let action: WakeAction = serde_json::from_str("\"command_mode\"").unwrap();
        assert_eq!(action, WakeAction::AwaitSubcommand);
    }

    #[test]
    fn audio_mode_u8_roundtrip() {
        for mode in [
            AudioMode::Off,
            AudioMode::Standby,
            AudioMode::Dictation,
            AudioMode::Processing,
            AudioMode::AwaitingSubcommand,
        ] {
            assert_eq!(AudioMode::from_u8(mode.as_u8()), mode);
        }
    }
}
