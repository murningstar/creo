//! Wake word and command detection via Google speech-embedding (96-dim) + DTW/centroid matching.
//!
//! Architecture: audio → melspectrogram.onnx → embedding_model.onnx → frame embeddings → DTW match.
//! Language-agnostic: works on audio level, no text transcription involved.
//! User records 3-5 samples per command; frame embeddings are pre-computed and stored.
//!
//! Detection modes:
//! - **DTW (primary):** Frame-level Dynamic Time Warping preserves temporal structure.
//!   Compares the full sequence of frame embeddings, not just the average.
//! - **Centroid (fallback):** Mean embedding + cosine similarity for old `.emb`-only samples.
//!
//! Models (from openWakeWord releases):
//! - melspectrogram.onnx (1.04 MB): raw audio → mel spectrogram
//!   Tensors: input "input" (batch, samples), output "output" (time, 1, dim, 32)
//! - embedding_model.onnx (1.27 MB): mel frames → 96-dim embedding
//!   Tensors: input "input_1" (batch, 76, 32, 1), output "conv2d_19" (batch, 1, 1, 96)

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use ndarray::Array1;
use ort::session::Session;
use ort::value::Tensor;

use dtw_rs::Solution;

use super::WakeAction;

// Google speech-embedding model constants
const EMBEDDING_DIM: usize = 96;
const EMBEDDING_WINDOW: usize = 76; // mel frames per embedding input
const EMBEDDING_STRIDE: usize = 8; // mel frames between consecutive embeddings
const MEL_BINS: usize = 32;

/// Minimum audio length in samples (16kHz) to guarantee at least one embedding.
/// 76 mel frames × 160 samples/frame (hop_length) + 512 (n_fft) = 12672.
/// Rounded up to 13000 for safety margin.
const MIN_AUDIO_SAMPLES: usize = 13000;

/// DTW: maximum normalized cosine distance for a match.
/// Each frame pair has cosine distance in [0, 2]. Normalized DTW divides by path length.
/// Calibrated from real data: true matches ≈ 0.03-0.07, false positives ≈ 0.19+.
/// 0.15 gives clear separation: true matches pass, random speech rejected.
const DTW_DISTANCE_THRESHOLD: f32 = 0.15;

/// Minimum number of embedding frames for DTW detection.
/// Wake words are multi-word phrases (~1-2s speech). Short inputs (<7 frames ≈ <600ms)
/// produce unreliable DTW distances and cause false positives.
const MIN_DETECTION_FRAMES: usize = 7;

/// DTW: Sakoe-Chiba band width. Limits how far alignment can deviate from diagonal.
/// For short sequences (5-15 frames), 3 allows ±3 frame drift (variable speaking speed).
/// Dynamically clamped to max(len_x, len_y) - 1 to avoid panic on short inputs.
const DTW_BAND_WIDTH: usize = 3;

/// Cosine similarity threshold for centroid fallback (old .emb-only samples).
const CENTROID_THRESHOLD: f32 = 0.7;

type FrameSequence = Vec<[f32; EMBEDDING_DIM]>;

pub struct DetectionResult {
    pub action: WakeAction,
    pub command_name: String,
    pub similarity: f32,
}

pub struct WakeWordDetector {
    mel_session: Session,
    emb_session: Session,
    /// Per-command frame sequences: each sample stored as a sequence of 96-dim frames.
    /// Used for DTW matching (primary path).
    frame_references: HashMap<String, Vec<FrameSequence>>,
    /// Per-command centroids: mean embedding across all samples.
    /// Used as fallback for old samples that only have .emb files.
    centroids: HashMap<String, Array1<f32>>,
    /// Action mapping: command name → WakeAction.
    action_map: HashMap<String, WakeAction>,
    references_dir: PathBuf,
}

impl WakeWordDetector {
    pub fn new(
        mel_model_path: &str,
        emb_model_path: &str,
        references_dir: &str,
    ) -> Result<Self> {
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

        let references_dir = PathBuf::from(references_dir);
        let frame_references = Self::load_all_frame_references(&references_dir)?;
        let centroids = Self::compute_all_centroids(&references_dir)?;
        let action_map = Self::load_action_map(&references_dir);

        let dtw_count = frame_references.values().filter(|v| !v.is_empty()).count();
        let centroid_only = centroids.len().saturating_sub(dtw_count);

        log::info!(
            "WakeWordDetector loaded: {} commands ({} DTW, {} centroid-only fallback)",
            centroids.len(),
            dtw_count,
            centroid_only,
        );
        for (name, _) in &centroids {
            let action = action_map
                .get(name)
                .map(|a| format!("{:?}", a))
                .unwrap_or_else(|| "unmapped".to_string());
            let mode = if frame_references.get(name).map(|v| !v.is_empty()).unwrap_or(false) {
                "DTW"
            } else {
                "centroid"
            };
            log::info!("  command '{}' → {} [{}]", name, action, mode);
        }

        Ok(Self {
            mel_session,
            emb_session,
            frame_references,
            centroids,
            action_map,
            references_dir,
        })
    }

    /// Detect command in audio segment (16kHz mono f32).
    /// Primary: DTW frame-level matching. Fallback: centroid cosine similarity.
    pub fn detect(&mut self, audio: &[f32]) -> Option<DetectionResult> {
        let input_frames = match self.extract_frame_embeddings(audio) {
            Ok(frames) if !frames.is_empty() => frames,
            Ok(_) => return None,
            Err(e) => {
                log::error!("Embedding extraction failed: {}", e);
                return None;
            }
        };

        if input_frames.len() < MIN_DETECTION_FRAMES {
            log::info!(
                "DTW detect: input {} frames < minimum {} — skipping (too short for wake word)",
                input_frames.len(),
                MIN_DETECTION_FRAMES,
            );
            return None;
        }

        let mut best: Option<(f32, WakeAction, String)> = None;

        log::info!("DTW detect: input {} frames", input_frames.len());

        for (name, _centroid) in &self.centroids {
            let cmd = match self.action_map.get(name) {
                Some(&c) => c,
                None => continue,
            };

            // Primary: DTW on frame sequences
            if let Some(ref_samples) = self.frame_references.get(name) {
                if !ref_samples.is_empty() {
                    let distances: Vec<f32> = ref_samples
                        .iter()
                        .map(|ref_frames| dtw_normalized_distance(&input_frames, ref_frames))
                        .collect();
                    let min_distance = distances.iter().copied().fold(f32::MAX, f32::min);

                    log::info!(
                        "  '{}' DTW distances: [{}] min={:.4} threshold={:.4} {}",
                        name,
                        distances.iter().map(|d| format!("{:.4}", d)).collect::<Vec<_>>().join(", "),
                        min_distance,
                        DTW_DISTANCE_THRESHOLD,
                        if min_distance < DTW_DISTANCE_THRESHOLD { "MATCH" } else { "reject" },
                    );

                    let similarity = 1.0 - min_distance;

                    if min_distance < DTW_DISTANCE_THRESHOLD {
                        if best.is_none() || similarity > best.as_ref().unwrap().0 {
                            best = Some((similarity, cmd, name.clone()));
                        }
                    }
                    continue;
                }
            }

            // Fallback: centroid cosine similarity (old .emb-only samples)
            if let Some(centroid) = self.centroids.get(name) {
                let mean_emb = match self.mean_from_frames(&input_frames) {
                    Some(m) => m,
                    None => continue,
                };
                let similarity = cosine_similarity(&mean_emb, centroid);
                log::info!(
                    "  '{}' centroid similarity={:.4} threshold={:.4} {}",
                    name,
                    similarity,
                    CENTROID_THRESHOLD,
                    if similarity > CENTROID_THRESHOLD { "MATCH" } else { "reject" },
                );
                if similarity > CENTROID_THRESHOLD {
                    if best.is_none() || similarity > best.as_ref().unwrap().0 {
                        best = Some((similarity, cmd, name.clone()));
                    }
                }
            }
        }

        if let Some((sim, action, ref name)) = best {
            log::info!(
                "Wake word ACCEPTED: {:?} '{}' (score: {:.4})",
                action,
                name,
                sim
            );
        } else {
            log::info!("Wake word: no match (all rejected)");
        }

        best.map(|(similarity, action, command_name)| DetectionResult {
            action,
            command_name,
            similarity,
        })
    }

    /// Extract a single mean embedding from audio (average of all frame embeddings).
    pub fn extract_mean_embedding(&mut self, audio: &[f32]) -> Result<Array1<f32>> {
        let embeddings = self.extract_frame_embeddings(audio)?;
        if embeddings.is_empty() {
            return Err(anyhow!("No embeddings extracted"));
        }

        let n = embeddings.len() as f32;
        let mut mean = Array1::<f32>::zeros(EMBEDDING_DIM);
        for emb in &embeddings {
            for i in 0..EMBEDDING_DIM {
                mean[i] += emb[i];
            }
        }
        mean /= n;

        // L2 normalize for cosine similarity
        let norm = mean.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-8 {
            mean /= norm;
        }

        Ok(mean)
    }

    /// Extract per-frame embeddings from audio (16kHz mono f32).
    /// Short audio is zero-padded to minimum length.
    fn extract_frame_embeddings(&mut self, audio: &[f32]) -> Result<FrameSequence> {
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

    /// Save a reference for a command: both frame sequence (.frames) and mean embedding (.emb).
    pub fn save_reference(&mut self, command_name: &str, audio: &[f32]) -> Result<PathBuf> {
        let frames = self.extract_frame_embeddings(audio)?;
        if frames.is_empty() {
            return Err(anyhow!("No frames extracted from audio"));
        }

        let cmd_dir = self.references_dir.join(command_name);
        fs::create_dir_all(&cmd_dir)?;

        // Find next sample index: max existing index + 1 (collision-safe)
        let idx = fs::read_dir(&cmd_dir)
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

        // Save frame sequence (.frames)
        let frames_path = cmd_dir.join(format!("sample_{}.frames", idx));
        Self::save_frames_file(&frames_path, &frames)?;
        log::info!(
            "Saved reference frames: {} ({} frames × {} dims)",
            frames_path.display(),
            frames.len(),
            EMBEDDING_DIM
        );

        // Also save mean embedding (.emb) for backward compatibility
        let emb_path = cmd_dir.join(format!("sample_{}.emb", idx));
        let mean_emb = self.mean_from_frames(&frames).unwrap();
        Self::save_emb_file(&emb_path, &mean_emb)?;

        // Update in-memory references
        self.frame_references
            .entry(command_name.to_string())
            .or_default()
            .push(frames);
        self.recompute_centroid(command_name)?;

        Ok(frames_path)
    }

    /// Save action mapping to config.json.
    pub fn save_action_mapping(&self, command_name: &str, action: WakeAction) -> Result<()> {
        let config_path = self.references_dir.join("config.json");
        let mut map: HashMap<String, WakeAction> = if config_path.exists() {
            let data = fs::read_to_string(&config_path)?;
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            HashMap::new()
        };
        map.insert(command_name.to_string(), action);
        fs::create_dir_all(&self.references_dir)?;
        fs::write(&config_path, serde_json::to_string_pretty(&map)?)?;
        Ok(())
    }

    /// Reload references from disk.
    pub fn reload_references(&mut self) -> Result<()> {
        self.frame_references = Self::load_all_frame_references(&self.references_dir)?;
        self.centroids = Self::compute_all_centroids(&self.references_dir)?;
        self.action_map = Self::load_action_map(&self.references_dir);
        Ok(())
    }

    pub fn has_references(&self) -> bool {
        !self.centroids.is_empty()
    }

    // --- Private helpers ---

    fn mean_from_frames(&self, frames: &[[f32; EMBEDDING_DIM]]) -> Option<Array1<f32>> {
        if frames.is_empty() {
            return None;
        }
        let n = frames.len() as f32;
        let mut mean = Array1::<f32>::zeros(EMBEDDING_DIM);
        for frame in frames {
            for i in 0..EMBEDDING_DIM {
                mean[i] += frame[i];
            }
        }
        mean /= n;
        let norm = mean.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-8 {
            mean /= norm;
        }
        Some(mean)
    }

    fn recompute_centroid(&mut self, command_name: &str) -> Result<()> {
        let cmd_dir = self.references_dir.join(command_name);
        let samples = Self::load_mean_embeddings(&cmd_dir)?;
        if samples.is_empty() {
            self.centroids.remove(command_name);
            return Ok(());
        }

        let mut centroid = Array1::<f32>::zeros(EMBEDDING_DIM);
        for sample in &samples {
            centroid += sample;
        }
        centroid /= samples.len() as f32;

        let norm = centroid.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-8 {
            centroid /= norm;
        }

        self.centroids.insert(command_name.to_string(), centroid);
        Ok(())
    }

    // --- File I/O: frame sequences (.frames) ---

    fn save_frames_file(path: &Path, frames: &[[f32; EMBEDDING_DIM]]) -> Result<()> {
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

    fn load_frames_file(path: &Path) -> Result<FrameSequence> {
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

    fn load_all_frame_references(
        dir: &Path,
    ) -> Result<HashMap<String, Vec<FrameSequence>>> {
        let mut all = HashMap::new();
        if !dir.exists() {
            return Ok(all);
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }

            let name = entry.file_name().to_string_lossy().to_string();
            let mut samples = Vec::new();

            if let Ok(files) = fs::read_dir(entry.path()) {
                for file in files.flatten() {
                    if file.path().extension().and_then(|e| e.to_str()) == Some("frames") {
                        match Self::load_frames_file(&file.path()) {
                            Ok(frames) => samples.push(frames),
                            Err(e) => {
                                log::warn!("Failed to load {}: {}", file.path().display(), e)
                            }
                        }
                    }
                }
            }

            if !samples.is_empty() {
                all.insert(name, samples);
            }
        }

        Ok(all)
    }

    // --- File I/O: mean embeddings (.emb) — backward compat ---

    fn save_emb_file(path: &Path, emb: &Array1<f32>) -> Result<()> {
        let mut data: Vec<u8> = Vec::new();
        let dim = EMBEDDING_DIM as u32;
        data.extend_from_slice(&dim.to_le_bytes());
        for &val in emb.iter() {
            data.extend_from_slice(&val.to_le_bytes());
        }
        fs::write(path, &data)?;
        Ok(())
    }

    fn compute_all_centroids(dir: &Path) -> Result<HashMap<String, Array1<f32>>> {
        let mut centroids = HashMap::new();
        if !dir.exists() {
            return Ok(centroids);
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }

            let name = entry.file_name().to_string_lossy().to_string();
            let samples = Self::load_mean_embeddings(&entry.path())?;
            if samples.is_empty() {
                continue;
            }

            let mut centroid = Array1::<f32>::zeros(EMBEDDING_DIM);
            for sample in &samples {
                centroid += sample;
            }
            centroid /= samples.len() as f32;

            let norm = centroid.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 1e-8 {
                centroid /= norm;
            }

            centroids.insert(name, centroid);
        }

        Ok(centroids)
    }

    fn load_mean_embeddings(dir: &Path) -> Result<Vec<Array1<f32>>> {
        let mut samples = Vec::new();
        if !dir.exists() {
            return Ok(samples);
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("emb") {
                continue;
            }

            match Self::load_embedding_file(&path) {
                Ok(emb) => samples.push(emb),
                Err(e) => log::warn!("Failed to load {}: {}", path.display(), e),
            }
        }

        Ok(samples)
    }

    fn load_embedding_file(path: &Path) -> Result<Array1<f32>> {
        let data = fs::read(path)?;
        if data.len() < 4 {
            return Err(anyhow!("File too small"));
        }

        let dim = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if dim != EMBEDDING_DIM {
            return Err(anyhow!(
                "Dim mismatch: expected {}, got {}",
                EMBEDDING_DIM,
                dim
            ));
        }

        let expected = 4 + dim * 4;
        if data.len() < expected {
            return Err(anyhow!("File truncated"));
        }

        let mut emb = Array1::<f32>::zeros(dim);
        for j in 0..dim {
            let offset = 4 + j * 4;
            emb[j] = f32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
        }

        Ok(emb)
    }

    // --- Config ---

    fn load_action_map(dir: &Path) -> HashMap<String, WakeAction> {
        let config_path = dir.join("config.json");
        if !config_path.exists() {
            return HashMap::new();
        }
        match fs::read_to_string(&config_path) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_else(|e| {
                log::warn!("Failed to parse wakewords config.json: {}", e);
                HashMap::new()
            }),
            Err(e) => {
                log::warn!("Failed to read wakewords config.json: {}", e);
                HashMap::new()
            }
        }
    }

    // --- ONNX inference ---

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
        anyhow::ensure!(mel_window.len() == EMBEDDING_WINDOW * MEL_BINS, "Mel window size mismatch: expected {}, got {}", EMBEDDING_WINDOW * MEL_BINS, mel_window.len());

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
/// Normalization: total_distance / (len_a + len_b) — standard across all production KWS systems
/// (Raven, Rustpotter, local-wake, Snips). More stable than path-length normalization
/// because it depends only on input lengths, not alignment quality.
fn dtw_normalized_distance(a: &[[f32; EMBEDDING_DIM]], b: &[[f32; EMBEDDING_DIM]]) -> f32 {
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
fn cosine_distance_frames(a: &[f32; EMBEDDING_DIM], b: &[f32; EMBEDDING_DIM]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    let denom = norm_a * norm_b;
    if denom < 1e-8 {
        return 1.0;
    }
    1.0 - dot / denom
}

/// Cosine similarity between two ndarray vectors (for centroid fallback).
fn cosine_similarity(a: &Array1<f32>, b: &Array1<f32>) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    let denom = norm_a * norm_b;
    if denom < 1e-8 {
        return 0.0;
    }
    dot / denom
}
