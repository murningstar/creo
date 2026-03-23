//! Voice Activity Detection via Silero VAD v6 (manual ONNX integration).
//!
//! Uses ort crate directly for ONNX inference. Context prepending and state
//! management handled manually.
//!
//! Adaptive threshold: tracks ambient noise level and raises speech threshold
//! dynamically to reduce false triggers in noisy environments.
//!
//! TODO: Switch to `silero-vad-rust` crate (v6.2+) when it resolves ndarray
//! version conflict with ort 2.0.0-rc.12. The crate bundles v6 models and
//! handles all internals automatically.
//! Track: https://github.com/sheldonix/silero-vad-rust/issues
//!
//! TODO: Monitor FireRedVAD (https://github.com/FireRedTeam/FireRedVAD) for Rust bindings.
//! FireRedVAD has F1 0.9757 vs Silero v6 F1 0.9595, and 2.69% FAR vs 9.41%.
//! No Rust crate exists yet (as of 2026-03). When one appears, evaluate switching.
//!
//! Silero VAD v5/v6 tensor interface (identical across both versions):
//! - Inputs: "input" (1, 576), "state" (2, 1, 128), "sr" scalar i64
//! - Outputs: "output" (1, 1) probability, "stateN" (2, 1, 128)
//! - Context: 64 samples prepended from previous chunk (critical for accuracy)
//! Verified: v6 model has same tensor names/shapes as v5 (inspected 2026-03-23).

use anyhow::{anyhow, Context, Result};
use ort::session::Session;
use ort::value::Tensor;

const CHUNK_SIZE: usize = 512; // 32ms at 16kHz
const CONTEXT_SIZE: usize = 64;
const INPUT_SIZE: usize = CONTEXT_SIZE + CHUNK_SIZE; // 576
const SAMPLE_RATE: i64 = 16000;
const STATE_DIM: usize = 2 * 1 * 128;

/// Base speech threshold (used in quiet environments).
const BASE_THRESHOLD: f32 = 0.5;

/// Maximum adaptive threshold (cap to prevent threshold from rising too high).
const MAX_THRESHOLD: f32 = 0.75;

/// Margin above ambient noise baseline for adaptive threshold.
/// If ambient prob averages 0.3, threshold becomes 0.3 + 0.15 = 0.45 (capped by BASE).
const ADAPTIVE_MARGIN: f32 = 0.15;

/// Exponential moving average factor for ambient noise tracking.
/// Lower = slower adaptation, more stable. 0.01 = ~100 chunks (~3.2 sec) to converge.
const AMBIENT_EMA_ALPHA: f32 = 0.01;

pub struct SileroVad {
    session: Session,
    state: Vec<f32>,
    context: Vec<f32>,
    sr: Tensor<i64>,

    /// Adaptive threshold: max(BASE_THRESHOLD, ambient_avg + ADAPTIVE_MARGIN)
    threshold: f32,

    /// Exponential moving average of VAD probabilities during non-speech.
    /// Tracks ambient noise level.
    ambient_avg: f32,
}

impl SileroVad {
    pub fn new(model_path: &str) -> Result<Self> {
        let session = Session::builder()
            .map_err(|e| anyhow!("{e}"))?
            .with_intra_threads(1)
            .map_err(|e| anyhow!("{e}"))?
            .commit_from_file(model_path)
            .context("Failed to load Silero VAD model")?;

        let sr = Tensor::from_array(([0usize; 0], vec![SAMPLE_RATE]))?;

        Ok(Self {
            session,
            state: vec![0.0f32; STATE_DIM],
            context: vec![0.0f32; CONTEXT_SIZE],
            sr,
            threshold: BASE_THRESHOLD,
            ambient_avg: 0.0,
        })
    }

    pub fn process_chunk(&mut self, chunk: &[f32]) -> Result<f32> {
        assert_eq!(chunk.len(), CHUNK_SIZE, "VAD chunk must be {} samples", CHUNK_SIZE);

        let mut input_data = Vec::with_capacity(INPUT_SIZE);
        input_data.extend_from_slice(&self.context);
        input_data.extend_from_slice(chunk);

        let input = Tensor::from_array(([1usize, INPUT_SIZE], input_data))?;
        let state = Tensor::from_array(([2usize, 1, 128], self.state.clone()))?;

        let outputs = self.session.run(ort::inputs! {
            "input" => input,
            "sr" => &self.sr,
            "state" => state,
        })?;

        let prob = {
            let (_, data) = outputs["output"].try_extract_tensor::<f32>()?;
            data[0]
        };

        {
            let (_, data) = outputs["stateN"].try_extract_tensor::<f32>()?;
            self.state.copy_from_slice(data);
        }

        self.context.copy_from_slice(&chunk[CHUNK_SIZE - CONTEXT_SIZE..]);

        Ok(prob)
    }

    pub fn is_speech(&mut self, chunk: &[f32]) -> Result<bool> {
        let prob = self.process_chunk(chunk)?;
        let is_speech = prob > self.threshold;

        // Update ambient noise average only during non-speech.
        // During speech, ambient level should not change.
        if !is_speech {
            self.ambient_avg = self.ambient_avg * (1.0 - AMBIENT_EMA_ALPHA) + prob * AMBIENT_EMA_ALPHA;

            // Adapt threshold: base or ambient + margin, whichever is higher.
            // Capped at MAX_THRESHOLD to prevent over-adaptation.
            let adaptive = self.ambient_avg + ADAPTIVE_MARGIN;
            self.threshold = BASE_THRESHOLD.max(adaptive).min(MAX_THRESHOLD);
        }

        Ok(is_speech)
    }

    pub fn reset(&mut self) {
        self.state.fill(0.0);
        self.context.fill(0.0);
        self.ambient_avg = 0.0;
        self.threshold = BASE_THRESHOLD;
    }

    pub fn chunk_size() -> usize {
        CHUNK_SIZE
    }

    /// Current adaptive threshold (for debugging/logging).
    pub fn current_threshold(&self) -> f32 {
        self.threshold
    }
}
