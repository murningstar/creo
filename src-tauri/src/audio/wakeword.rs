//! Wake word and command detection via Google speech-embedding (96-dim) + SVM classifier.
//!
//! Architecture: audio → melspectrogram.onnx → embedding_model.onnx → mean embedding → SVM classify.
//! Language-agnostic: works on audio level, no text transcription involved.
//! User records 3-5 samples per command; embeddings are pre-computed and stored.
//! SVM is retrained on-device in ~200ms when commands change.
//!
//! Models (from openWakeWord releases):
//! - melspectrogram.onnx (1.04 MB): raw audio → mel spectrogram
//!   Tensors: input "input" (batch, samples), output "output" (time, 1, dim, 32)
//! - embedding_model.onnx (1.27 MB): mel frames → 96-dim embedding
//!   Tensors: input "input_1" (batch, 76, 32, 1), output "conv2d_19" (batch, 1, 1, 96)
//!
//! Reference: local-wake (st-matskevich), livekit-wakeword (livekit).
//! These models and tensor interfaces are specific to Google's speech-embedding CNN.
//! A different embedding model will require different preprocessing and tensor shapes.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use ndarray::{Array1, Array2};
use ort::session::Session;
use ort::value::Tensor;

use super::WakeCommand;

// Google speech-embedding model constants
const EMBEDDING_DIM: usize = 96;
const EMBEDDING_WINDOW: usize = 76; // mel frames per embedding input
const EMBEDDING_STRIDE: usize = 8; // mel frames between consecutive embeddings
const MEL_BINS: usize = 32;

/// Minimum audio length in samples (16kHz) to guarantee at least one embedding.
/// 76 mel frames × 160 samples/frame (hop_length) + 512 (n_fft) = 12672.
/// Rounded up to 13000 for safety margin.
const MIN_AUDIO_SAMPLES: usize = 13000;

/// Cosine similarity threshold for nearest-centroid fallback (used when SVM not available).
const CENTROID_THRESHOLD: f32 = 0.7;

pub struct WakeWordDetector {
    mel_session: Session,
    emb_session: Session,
    /// Per-command centroids: mean embedding across all samples.
    centroids: HashMap<String, Array1<f32>>,
    /// Action mapping: command name → WakeAction.
    action_map: HashMap<String, WakeCommand>,
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
        let centroids = Self::compute_all_centroids(&references_dir)?;
        let action_map = Self::load_action_map(&references_dir);

        log::info!(
            "WakeWordDetector loaded: {} commands (centroid-based)",
            centroids.len(),
        );
        for (name, _) in &centroids {
            let action = action_map.get(name).map(|a| format!("{:?}", a)).unwrap_or_else(|| "unmapped".to_string());
            log::info!("  command '{}' → {}", name, action);
        }

        Ok(Self {
            mel_session,
            emb_session,
            centroids,
            action_map,
            references_dir,
        })
    }

    /// Detect command in audio segment (16kHz mono f32).
    /// Uses cosine similarity to nearest centroid.
    pub fn detect(&mut self, audio: &[f32]) -> Option<WakeCommand> {
        let embedding = match self.extract_mean_embedding(audio) {
            Ok(emb) => emb,
            Err(e) => {
                log::error!("Embedding extraction failed: {}", e);
                return None;
            }
        };

        let mut best: Option<(f32, WakeCommand)> = None;

        for (name, centroid) in &self.centroids {
            let similarity = cosine_similarity(&embedding, centroid);

            let cmd = match self.action_map.get(name) {
                Some(&c) => c,
                None => continue,
            };

            if similarity > CENTROID_THRESHOLD {
                if best.is_none() || similarity > best.unwrap().0 {
                    best = Some((similarity, cmd));
                }
            }
        }

        if let Some((sim, cmd)) = best {
            log::info!("Command detected: {:?} (similarity: {:.4})", cmd, sim);
        }

        best.map(|(_, cmd)| cmd)
    }

    /// Extract a single mean embedding from audio (average of all frame embeddings).
    pub fn extract_mean_embedding(&mut self, audio: &[f32]) -> Result<Array1<f32>> {
        let embeddings = self.extract_frame_embeddings(audio)?;
        if embeddings.is_empty() {
            return Err(anyhow!("No embeddings extracted"));
        }

        // Average all frame embeddings into one 96-dim vector
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
    fn extract_frame_embeddings(&mut self, audio: &[f32]) -> Result<Vec<[f32; EMBEDDING_DIM]>> {
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

        log::info!(
            "Embedding extraction: {} audio samples → {} mel frames (need >= {})",
            audio.len(), n_frames, EMBEDDING_WINDOW
        );

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

    /// Save a reference embedding to disk for a command.
    pub fn save_reference(&mut self, command_name: &str, audio: &[f32]) -> Result<PathBuf> {
        let mean_emb = self.extract_mean_embedding(audio)?;

        let cmd_dir = self.references_dir.join(command_name);
        fs::create_dir_all(&cmd_dir)?;

        // Find next sample index
        let count = fs::read_dir(&cmd_dir)
            .map(|entries| entries.filter_map(|e| e.ok()).count())
            .unwrap_or(0);

        let path = cmd_dir.join(format!("sample_{}.emb", count));

        // Serialize as flat f32 array with header (n_dims)
        let mut data: Vec<u8> = Vec::new();
        let dim = EMBEDDING_DIM as u32;
        data.extend_from_slice(&dim.to_le_bytes());
        for &val in mean_emb.iter() {
            data.extend_from_slice(&val.to_le_bytes());
        }

        fs::write(&path, &data)?;
        log::info!("Saved reference: {} ({} dims)", path.display(), dim);

        // Recompute centroid for this command
        self.recompute_centroid(command_name)?;

        Ok(path)
    }

    /// Save action mapping to config.json.
    pub fn save_action_mapping(&self, command_name: &str, action: WakeCommand) -> Result<()> {
        let config_path = self.references_dir.join("config.json");
        let mut map: HashMap<String, WakeCommand> = if config_path.exists() {
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
        self.centroids = Self::compute_all_centroids(&self.references_dir)?;
        self.action_map = Self::load_action_map(&self.references_dir);
        Ok(())
    }

    pub fn has_references(&self) -> bool {
        !self.centroids.is_empty()
    }

    // --- Private ---

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

        // L2 normalize
        let norm = centroid.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-8 {
            centroid /= norm;
        }

        self.centroids.insert(command_name.to_string(), centroid);
        Ok(())
    }

    fn compute_all_centroids(dir: &Path) -> Result<HashMap<String, Array1<f32>>> {
        let mut centroids = HashMap::new();

        if !dir.exists() {
            log::info!("Wakewords directory not found: {}", dir.display());
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

            // L2 normalize
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
            return Err(anyhow!("Dim mismatch: expected {}, got {}", EMBEDDING_DIM, dim));
        }

        let expected = 4 + dim * 4;
        if data.len() < expected {
            return Err(anyhow!("File truncated"));
        }

        let mut emb = Array1::<f32>::zeros(dim);
        for j in 0..dim {
            let offset = 4 + j * 4;
            emb[j] = f32::from_le_bytes([
                data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
            ]);
        }

        Ok(emb)
    }

    fn load_action_map(dir: &Path) -> HashMap<String, WakeCommand> {
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
        assert_eq!(mel_window.len(), EMBEDDING_WINDOW * MEL_BINS);

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

// --- Utilities ---

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
