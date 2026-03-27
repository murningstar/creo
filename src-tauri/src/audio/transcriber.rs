use anyhow::{Context, Result};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Maximum characters of previous transcription to carry as context.
/// Limits hallucination risk from long prompt. ~200 chars ≈ 30-50 words.
const MAX_CONTEXT_CHARS: usize = 200;

/// Segments with no_speech_probability above this are discarded (likely silence/noise hallucination).
const NO_SPEECH_PROB_THRESHOLD: f32 = 0.6;

/// Minimum meaningful text length. Segments shorter than this are likely garbage.
const MIN_SEGMENT_TEXT_LEN: usize = 3;

/// Whisper-based transcriber for dictation (continuous speech-to-text).
/// Maintains context between segments via initial_prompt for cross-segment coherence.
pub struct Transcriber {
    ctx: WhisperContext,
    /// Previous transcription text, used as context for next segment.
    prev_context: String,
}

impl Transcriber {
    pub fn new(model_path: &str) -> Result<Self> {
        let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
            .context("Failed to load whisper model")?;
        Ok(Self {
            ctx,
            prev_context: String::new(),
        })
    }

    /// Transcribe audio buffer (16kHz mono f32) to text.
    /// Uses previous transcription as context for cross-segment coherence.
    pub fn transcribe(&mut self, audio: &[f32], lang: &str) -> Result<String> {
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
        params.set_no_context(false);

        // Pass previous transcription as context for cross-segment coherence
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
                // Hallucination filter: skip segments that are likely non-speech
                let no_speech_prob = segment.no_speech_probability();
                if no_speech_prob > NO_SPEECH_PROB_THRESHOLD {
                    log::info!(
                        "Segment {} discarded: no_speech_prob={:.3} > {:.1}",
                        i,
                        no_speech_prob,
                        NO_SPEECH_PROB_THRESHOLD,
                    );
                    continue;
                }

                match segment.to_str_lossy() {
                    Ok(s) => {
                        let segment_text = s.trim();
                        // Skip very short segments (likely garbage)
                        if segment_text.len() >= MIN_SEGMENT_TEXT_LEN {
                            text.push_str(&s);
                        } else if !segment_text.is_empty() {
                            log::info!(
                                "Segment {} discarded: text '{}' too short (<{} chars)",
                                i,
                                segment_text,
                                MIN_SEGMENT_TEXT_LEN,
                            );
                        }
                    }
                    Err(_) => {}
                }
            }
        }

        let trimmed = text.trim().to_string();

        // Update context for next call (truncate to limit hallucination risk)
        if !trimmed.is_empty() {
            self.prev_context = if trimmed.len() > MAX_CONTEXT_CHARS {
                trimmed[trimmed.len() - MAX_CONTEXT_CHARS..].to_string()
            } else {
                trimmed.clone()
            };
        }

        Ok(trimmed)
    }

    /// Reset context (e.g., when leaving dictation mode).
    pub fn reset_context(&mut self) {
        self.prev_context.clear();
    }
}
