use anyhow::{Context, Result};
use strsim::normalized_levenshtein;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use super::WakeCommand;

const WAKE_THRESHOLD: f64 = 0.55;

const WAKE_PATTERNS: &[(&str, WakeCommand)] = &[
    ("крео приём", WakeCommand::Priem),
    ("крео прием", WakeCommand::Priem),
    ("крео приём.", WakeCommand::Priem),
    ("крео прием.", WakeCommand::Priem),
    ("крео, приём", WakeCommand::Priem),
    ("крео, прием", WakeCommand::Priem),
    ("крио приём", WakeCommand::Priem),
    ("крио прием", WakeCommand::Priem),
    ("крео вписывай", WakeCommand::Vpisyvai),
    ("крео, вписывай", WakeCommand::Vpisyvai),
    ("крио вписывай", WakeCommand::Vpisyvai),
    ("крео готово", WakeCommand::Gotovo),
    ("крео, готово", WakeCommand::Gotovo),
    ("крио готово", WakeCommand::Gotovo),
];

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

/// Match transcribed text against known wake word patterns using fuzzy matching.
pub fn match_wake_word(text: &str) -> Option<WakeCommand> {
    let normalized = text
        .to_lowercase()
        .trim()
        .replace(',', "")
        .replace('.', "");
    let normalized = normalized.trim();

    if normalized.is_empty() {
        return None;
    }

    WAKE_PATTERNS
        .iter()
        .map(|(pattern, cmd)| {
            let clean_pattern = pattern.replace(',', "").replace('.', "");
            let score = normalized_levenshtein(normalized, clean_pattern.trim());
            (score, *cmd)
        })
        .filter(|(score, _)| *score >= WAKE_THRESHOLD)
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(_, cmd)| cmd)
}
