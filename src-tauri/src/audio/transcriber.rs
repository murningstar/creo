use anyhow::{Context, Result};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Whisper-based transcriber for dictation (continuous speech-to-text).
/// Wake word detection is handled by wakeword.rs (embedding+DTW), not here.
pub struct Transcriber {
    ctx: WhisperContext,
}

impl Transcriber {
    pub fn new(model_path: &str) -> Result<Self> {
        let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
            .context("Failed to load whisper model")?;
        Ok(Self { ctx })
    }

    /// Transcribe audio buffer (16kHz mono f32) to text.
    pub fn transcribe(&self, audio: &[f32], lang: &str) -> Result<String> {
        let mut state = self
            .ctx
            .create_state()
            .context("Failed to create whisper state")?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some(lang));
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_single_segment(false);
        params.set_no_context(true);

        state
            .full(params, audio)
            .context("Whisper transcription failed")?;

        let n_segments = state.full_n_segments();
        let mut text = String::new();
        for i in 0..n_segments {
            if let Some(segment) = state.get_segment(i) {
                match segment.to_str_lossy() {
                    Ok(s) => text.push_str(&s),
                    Err(_) => {}
                }
            }
        }

        Ok(text.trim().to_string())
    }
}
