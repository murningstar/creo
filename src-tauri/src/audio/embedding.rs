//! Shared embedding extraction and DTW matching utilities.
//!
//! Extracted from `wakeword.rs` to be reused by both wake word detection and subcommand cascade.
//!
//! Pipeline: audio (16kHz f32) → melspectrogram.onnx → embedding_model.onnx → 96-dim frame embeddings → DTW match.

use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use ort::session::Session;
use ort::value::Tensor;

use dtw_rs::Solution;

// Google speech-embedding model constants
pub const EMBEDDING_DIM: usize = 96;
pub const EMBEDDING_WINDOW: usize = 76; // mel frames per embedding input
pub const EMBEDDING_STRIDE: usize = 8; // mel frames between consecutive embeddings
pub const MEL_BINS: usize = 32;

/// Minimum audio length in samples (16kHz) to guarantee at least one embedding.
/// 76 mel frames × 160 samples/frame (hop_length) + 512 (n_fft) = 12672.
/// Rounded up to 13000 for safety margin.
pub const MIN_AUDIO_SAMPLES: usize = 13000;

/// DTW: maximum normalized cosine distance for a match.
/// Calibrated from real data: true matches ≈ 0.03-0.07, false positives ≈ 0.19+.
pub const DTW_DISTANCE_THRESHOLD: f32 = 0.15;

/// Minimum number of embedding frames for reliable DTW detection.
/// Short inputs (<7 frames ≈ <600ms) produce unreliable distances and false positives.
pub const MIN_DETECTION_FRAMES: usize = 7;

/// DTW: Sakoe-Chiba band width. Limits alignment deviation from diagonal.
/// For short sequences (5-15 frames), 3 allows ±3 frame drift (variable speaking speed).
const DTW_BAND_WIDTH: usize = 3;

pub type FrameSequence = Vec<[f32; EMBEDDING_DIM]>;

/// ONNX-based embedding extractor: audio → mel spectrogram → 96-dim frame embeddings.
pub struct EmbeddingExtractor {
    mel_session: Session,
    emb_session: Session,
}

impl EmbeddingExtractor {
    pub fn new(mel_model_path: &str, emb_model_path: &str) -> Result<Self> {
        let mel_session = Session::builder()
            .map_err(|e| anyhow!("{e}"))?
            .with_intra_threads(1)
            .map_err(|e| anyhow!("{e}"))?
            .commit_from_file(mel_model_path)
            .context("Failed to load melspectrogram model")?;

        let emb_session = Session::builder()
            .map_err(|e| anyhow!("{e}"))?
            .with_intra_threads(1)
            .map_err(|e| anyhow!("{e}"))?
            .commit_from_file(emb_model_path)
            .context("Failed to load embedding model")?;

        Ok(Self {
            mel_session,
            emb_session,
        })
    }

    /// Extract per-frame embeddings from audio (16kHz mono f32).
    /// Short audio is zero-padded to minimum length.
    pub fn extract_frame_embeddings(&mut self, audio: &[f32]) -> Result<FrameSequence> {
        // Pad short audio with silence
        let padded;
        let audio = if audio.len() < MIN_AUDIO_SAMPLES {
            padded = {
                let mut buf = audio.to_vec();
                buf.resize(MIN_AUDIO_SAMPLES, 0.0);
                buf
            };
            &padded
        } else {
            audio
        };

        // Step 1: mel spectrogram
        let mel = self.compute_mel(audio)?;
        let n_frames = mel.len() / MEL_BINS;

        if n_frames < EMBEDDING_WINDOW {
            return Ok(Vec::new());
        }

        // Step 2: sliding window → embeddings
        let mut embeddings = Vec::new();
        let mut offset = 0;

        while offset + EMBEDDING_WINDOW <= n_frames {
            let start = offset * MEL_BINS;
            let end = (offset + EMBEDDING_WINDOW) * MEL_BINS;
            let window = &mel[start..end];

            let emb = self.compute_embedding(window)?;
            embeddings.push(emb);

            offset += EMBEDDING_STRIDE;
        }

        Ok(embeddings)
    }

    fn compute_mel(&mut self, audio: &[f32]) -> Result<Vec<f32>> {
        let input = Tensor::from_array(([1usize, audio.len()], audio.to_vec()))?;

        let outputs = self.mel_session.run(ort::inputs! {
            "input" => input,
        })?;

        let (_, raw) = outputs[0].try_extract_tensor::<f32>()?;

        // Post-process: x / 10.0 + 2.0 (from openWakeWord reference)
        let mel: Vec<f32> = raw.iter().map(|&x| x / 10.0 + 2.0).collect();

        Ok(mel)
    }

    fn compute_embedding(&mut self, mel_window: &[f32]) -> Result<[f32; EMBEDDING_DIM]> {
        anyhow::ensure!(
            mel_window.len() == EMBEDDING_WINDOW * MEL_BINS,
            "Mel window size mismatch: expected {}, got {}",
            EMBEDDING_WINDOW * MEL_BINS,
            mel_window.len()
        );

        let input = Tensor::from_array(
            ([1usize, EMBEDDING_WINDOW, MEL_BINS, 1], mel_window.to_vec()),
        )?;

        let outputs = self.emb_session.run(ort::inputs! {
            "input_1" => input,
        })?;

        let (_, raw) = outputs["conv2d_19"].try_extract_tensor::<f32>()?;

        let mut emb = [0.0f32; EMBEDDING_DIM];
        emb.copy_from_slice(&raw[..EMBEDDING_DIM]);

        Ok(emb)
    }
}

// --- DTW utilities ---

/// Compute normalized DTW distance between two frame sequences using cosine distance.
/// Normalization: total_distance / (len_a + len_b) — standard across production KWS systems.
pub fn dtw_normalized_distance(a: &[[f32; EMBEDDING_DIM]], b: &[[f32; EMBEDDING_DIM]]) -> f32 {
    let max_len = a.len().max(b.len());
    let norm = (a.len() + b.len()).max(1) as f32;

    // Sakoe-Chiba requires band < max(len_x, len_y). Fall back to unconstrained DTW for short sequences.
    if max_len <= DTW_BAND_WIDTH {
        let result =
            dtw_rs::dtw_with_distance(a, b, |x, y| cosine_distance_frames(x, y));
        return result.distance() / norm;
    }

    let result = dtw_rs::sakoe_chiba_with_distance(a, b, DTW_BAND_WIDTH, |x, y| {
        cosine_distance_frames(x, y)
    });
    result.distance() / norm
}

/// Cosine distance between two 96-dim frame embeddings: 1.0 - cosine_similarity.
pub fn cosine_distance_frames(a: &[f32; EMBEDDING_DIM], b: &[f32; EMBEDDING_DIM]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    let denom = norm_a * norm_b;
    if denom < 1e-8 {
        return 1.0;
    }
    1.0 - dot / denom
}

// --- File I/O for .frames files ---

/// Save a frame sequence to a binary `.frames` file.
/// Format: [n_frames: u32 LE][n_dims: u32 LE][frame0_val0: f32 LE][frame0_val1: f32 LE]...
pub fn save_frames_file(path: &Path, frames: &[[f32; EMBEDDING_DIM]]) -> Result<()> {
    let mut data: Vec<u8> = Vec::new();
    let n_frames = frames.len() as u32;
    let n_dims = EMBEDDING_DIM as u32;
    data.extend_from_slice(&n_frames.to_le_bytes());
    data.extend_from_slice(&n_dims.to_le_bytes());
    for frame in frames {
        for &val in frame {
            data.extend_from_slice(&val.to_le_bytes());
        }
    }
    fs::write(path, &data)?;
    Ok(())
}

/// Load a frame sequence from a binary `.frames` file.
pub fn load_frames_file(path: &Path) -> Result<FrameSequence> {
    let data = fs::read(path)?;
    if data.len() < 8 {
        return Err(anyhow!("Frames file too small"));
    }

    let n_frames = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let n_dims = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;

    if n_dims != EMBEDDING_DIM {
        return Err(anyhow!(
            "Dims mismatch: expected {}, got {}",
            EMBEDDING_DIM,
            n_dims
        ));
    }

    let expected = 8 + n_frames * n_dims * 4;
    if data.len() < expected {
        return Err(anyhow!("Frames file truncated"));
    }

    let mut frames = Vec::with_capacity(n_frames);
    for i in 0..n_frames {
        let mut frame = [0.0f32; EMBEDDING_DIM];
        for j in 0..n_dims {
            let offset = 8 + (i * n_dims + j) * 4;
            frame[j] = f32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
        }
        frames.push(frame);
    }

    Ok(frames)
}
