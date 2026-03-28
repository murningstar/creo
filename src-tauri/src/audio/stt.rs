//! STT engine abstraction for dictation.
//!
//! Two user-selectable engines:
//! - `WhisperEngine` — whisper-rs (whisper.cpp), supports 99 languages, NVIDIA GPU + CPU
//! - `ParakeetEngine` — parakeet-rs (ONNX Runtime), 25 EU languages, DirectML/CUDA/CPU, native punctuation
//!
//! Pipeline uses `Box<dyn DictationEngine>` — engine-agnostic.

use anyhow::{Context, Result};

/// Result of a dictation transcription.
pub struct DictationResult {
    pub text: String,
}

/// Engine-agnostic trait for dictation STT.
/// Implementations must be `Send` (used in transcription thread).
pub trait DictationEngine: Send {
    /// Transcribe audio buffer (16kHz mono f32) to text.
    fn transcribe(&mut self, audio: &[f32]) -> Result<DictationResult>;

    /// Reset any accumulated context (called when leaving dictation mode).
    fn reset_context(&mut self);

    /// Human-readable engine name for logging.
    fn name(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Whisper engine (whisper-rs / whisper.cpp)
// ---------------------------------------------------------------------------

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

const WHISPER_MAX_CONTEXT_CHARS: usize = 200;
const WHISPER_NO_SPEECH_PROB_THRESHOLD: f32 = 0.6;
const WHISPER_MIN_SEGMENT_TEXT_LEN: usize = 3;

pub struct WhisperEngine {
    ctx: WhisperContext,
    prev_context: String,
    language: String,
}

impl WhisperEngine {
    pub fn new(model_path: &str, language: &str) -> Result<Self> {
        let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
            .context("Failed to load Whisper model")?;
        Ok(Self {
            ctx,
            prev_context: String::new(),
            language: language.to_string(),
        })
    }
}

impl DictationEngine for WhisperEngine {
    fn transcribe(&mut self, audio: &[f32]) -> Result<DictationResult> {
        let mut state = self
            .ctx
            .create_state()
            .context("Failed to create Whisper state")?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some(&self.language));
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_single_segment(false);
        params.set_no_context(false);

        if !self.prev_context.is_empty() {
            params.set_initial_prompt(&self.prev_context);
        }

        state
            .full(params, audio)
            .context("Whisper transcription failed")?;

        let n_segments = state.full_n_segments();
        let mut text = String::new();
        for i in 0..n_segments {
            if let Some(segment) = state.get_segment(i) {
                let no_speech_prob = segment.no_speech_probability();
                if no_speech_prob > WHISPER_NO_SPEECH_PROB_THRESHOLD {
                    log::info!(
                        "Whisper segment {} discarded: no_speech_prob={:.3}",
                        i,
                        no_speech_prob,
                    );
                    continue;
                }

                if let Ok(s) = segment.to_str_lossy() {
                    let segment_text = s.trim();
                    if segment_text.len() >= WHISPER_MIN_SEGMENT_TEXT_LEN {
                        text.push_str(&s);
                    } else if !segment_text.is_empty() {
                        log::info!(
                            "Whisper segment {} discarded: '{}' too short",
                            i,
                            segment_text,
                        );
                    }
                }
            }
        }

        let trimmed = text.trim().to_string();

        if !trimmed.is_empty() {
            self.prev_context = if trimmed.len() > WHISPER_MAX_CONTEXT_CHARS {
                trimmed[trimmed.len() - WHISPER_MAX_CONTEXT_CHARS..].to_string()
            } else {
                trimmed.clone()
            };
        }

        Ok(DictationResult { text: trimmed })
    }

    fn reset_context(&mut self) {
        self.prev_context.clear();
    }

    fn name(&self) -> &str {
        "Whisper"
    }
}

// ---------------------------------------------------------------------------
// Parakeet engine (parakeet-rs / ONNX Runtime)
// ---------------------------------------------------------------------------

use parakeet_rs::{ParakeetTDT, Transcriber as ParakeetTranscriber};

pub struct ParakeetEngine {
    model: ParakeetTDT,
}

impl ParakeetEngine {
    pub fn new(model_dir: &str) -> Result<Self> {
        let model = ParakeetTDT::from_pretrained(model_dir, None)
            .context("Failed to load Parakeet TDT model")?;
        Ok(Self { model })
    }
}

impl DictationEngine for ParakeetEngine {
    fn transcribe(&mut self, audio: &[f32]) -> Result<DictationResult> {
        let result = self
            .model
            .transcribe_samples(audio.to_vec(), 16000, 1, None)
            .context("Parakeet transcription failed")?;

        let text = result.text.trim().to_string();
        Ok(DictationResult { text })
    }

    fn reset_context(&mut self) {
        // Parakeet TDT is a transducer — no explicit context to reset.
        // Each chunk is independent with good quality by design.
    }

    fn name(&self) -> &str {
        "Parakeet"
    }
}
