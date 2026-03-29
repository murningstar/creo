//! Subcommand cascade: tiered recognition for voice commands after "Крео, приём".
//!
//! Architecture:
//! - Tier 1 (DTW): Embedding-level matching for fixed subcommands (<50ms)
//! - Tier 2 (Vosk): Grammar-constrained STT for text subcommands (<100ms) — future
//! - Tier 3 (Qwen3): LLM + GBNF for parametric commands (0.5-2s) — future

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use super::embedding::{
    dtw_normalized_distance, EmbeddingExtractor, FrameSequence,
    DTW_DISTANCE_THRESHOLD, EMBEDDING_DIM, MIN_DETECTION_FRAMES,
};

// --- Manifest types (persisted as subcommands/manifest.json) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubcommandManifest {
    pub commands: Vec<SubcommandDef>,
}

impl Default for SubcommandManifest {
    fn default() -> Self {
        Self {
            commands: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubcommandDef {
    pub name: String,
    pub action: String,
    pub tier: SubcommandTierKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phrases: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<ParametricTemplate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubcommandTierKind {
    #[serde(rename = "dtw")]
    Dtw,
    #[serde(rename = "vosk")]
    Vosk,
    #[serde(rename = "llm")]
    Llm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParametricTemplate {
    pub pattern: String,
    pub slots: Vec<SlotDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotDef {
    pub name: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<String>,
}

// --- Match result ---

#[derive(Debug, Clone, Serialize)]
pub struct SubcommandMatch {
    #[serde(rename = "commandName")]
    pub command_name: String,
    pub action: String,
    pub confidence: f32,
    pub tier: u8,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub params: HashMap<String, String>,
}

// --- Cascade trait ---

pub trait SubcommandTier: Send {
    /// Try to match audio against this tier's commands.
    /// Returns None if no match (cascade continues to next tier).
    fn try_match(&mut self, audio: &[f32]) -> Option<SubcommandMatch>;

    /// Reload command definitions from updated manifest.
    fn reload(&mut self, commands: &[SubcommandDef], subcommands_dir: &Path) -> Result<()>;

    /// Human-readable tier name for logging.
    fn name(&self) -> &str;
}

// --- Tier 1: DTW embedding matching ---

pub struct DtwTier {
    extractor: EmbeddingExtractor,
    /// command_name → Vec<FrameSequence>
    references: HashMap<String, Vec<FrameSequence>>,
    /// command_name → action string
    actions: HashMap<String, String>,
}

impl DtwTier {
    pub fn new(
        mel_model_path: &str,
        emb_model_path: &str,
        subcommands_dir: &Path,
        manifest: &SubcommandManifest,
    ) -> Result<Self> {
        let extractor = EmbeddingExtractor::new(mel_model_path, emb_model_path)?;
        let mut tier = Self {
            extractor,
            references: HashMap::new(),
            actions: HashMap::new(),
        };
        tier.load_references(manifest, subcommands_dir)?;
        Ok(tier)
    }

    fn load_references(
        &mut self,
        manifest: &SubcommandManifest,
        subcommands_dir: &Path,
    ) -> Result<()> {
        self.references.clear();
        self.actions.clear();

        for cmd in &manifest.commands {
            if cmd.tier != SubcommandTierKind::Dtw {
                continue;
            }

            let cmd_dir = subcommands_dir.join(&cmd.name);
            if !cmd_dir.exists() {
                continue;
            }

            let mut samples = Vec::new();
            if let Ok(files) = fs::read_dir(&cmd_dir) {
                for file in files.flatten() {
                    if file.path().extension().and_then(|e| e.to_str()) == Some("frames") {
                        match load_frames_file(&file.path()) {
                            Ok(frames) => samples.push(frames),
                            Err(e) => {
                                log::warn!("Failed to load {}: {}", file.path().display(), e)
                            }
                        }
                    }
                }
            }

            if !samples.is_empty() {
                log::info!(
                    "DtwTier: loaded '{}' → action='{}' ({} samples)",
                    cmd.name,
                    cmd.action,
                    samples.len()
                );
                self.references.insert(cmd.name.clone(), samples);
                self.actions.insert(cmd.name.clone(), cmd.action.clone());
            }
        }

        Ok(())
    }
}

impl SubcommandTier for DtwTier {
    fn try_match(&mut self, audio: &[f32]) -> Option<SubcommandMatch> {
        if self.references.is_empty() {
            return None;
        }

        let input_frames = match self.extractor.extract_frame_embeddings(audio) {
            Ok(frames) if !frames.is_empty() => frames,
            Ok(_) => return None,
            Err(e) => {
                log::error!("DtwTier embedding extraction failed: {}", e);
                return None;
            }
        };

        if input_frames.len() < MIN_DETECTION_FRAMES {
            return None;
        }

        let mut best: Option<(f32, String)> = None;

        for (name, ref_samples) in &self.references {
            let distances: Vec<f32> = ref_samples
                .iter()
                .map(|ref_frames| dtw_normalized_distance(&input_frames, ref_frames))
                .collect();
            let min_distance = distances.iter().copied().fold(f32::MAX, f32::min);
            let similarity = 1.0 - min_distance;

            log::info!(
                "  DtwTier '{}' min_distance={:.4} threshold={:.4} {}",
                name,
                min_distance,
                DTW_DISTANCE_THRESHOLD,
                if min_distance < DTW_DISTANCE_THRESHOLD { "MATCH" } else { "reject" },
            );

            if min_distance < DTW_DISTANCE_THRESHOLD {
                if best.is_none() || similarity > best.as_ref().unwrap().0 {
                    best = Some((similarity, name.clone()));
                }
            }
        }

        best.map(|(confidence, command_name)| {
            let action = self
                .actions
                .get(&command_name)
                .cloned()
                .unwrap_or_default();
            SubcommandMatch {
                command_name,
                action,
                confidence,
                tier: 1,
                params: HashMap::new(),
            }
        })
    }

    fn reload(&mut self, commands: &[SubcommandDef], subcommands_dir: &Path) -> Result<()> {
        let manifest = SubcommandManifest {
            commands: commands.to_vec(),
        };
        self.load_references(&manifest, subcommands_dir)
    }

    fn name(&self) -> &str {
        "DTW"
    }
}

// --- Cascade runner ---

pub struct SubcommandCascade {
    tiers: Vec<Box<dyn SubcommandTier>>,
    manifest: SubcommandManifest,
    subcommands_dir: PathBuf,
}

impl SubcommandCascade {
    pub fn new(
        subcommands_dir: &str,
        mel_model_path: &str,
        emb_model_path: &str,
    ) -> Result<Self> {
        let subcommands_dir = PathBuf::from(subcommands_dir);
        let manifest = Self::load_manifest(&subcommands_dir);

        let dtw_tier = DtwTier::new(
            mel_model_path,
            emb_model_path,
            &subcommands_dir,
            &manifest,
        )?;

        let dtw_count = dtw_tier.references.len();
        log::info!(
            "SubcommandCascade loaded: {} DTW commands",
            dtw_count,
        );

        Ok(Self {
            tiers: vec![Box::new(dtw_tier)],
            manifest,
            subcommands_dir,
        })
    }

    /// Run the cascade: try each tier in order, return first match.
    pub fn process(&mut self, audio: &[f32]) -> Option<SubcommandMatch> {
        for tier in &mut self.tiers {
            if let Some(m) = tier.try_match(audio) {
                log::info!(
                    "Subcommand matched: '{}' via {} (confidence: {:.4}, action: '{}')",
                    m.command_name,
                    tier.name(),
                    m.confidence,
                    m.action,
                );
                return Some(m);
            }
        }
        None
    }

    pub fn reload(&mut self) -> Result<()> {
        self.manifest = Self::load_manifest(&self.subcommands_dir);
        for tier in &mut self.tiers {
            if let Err(e) = tier.reload(&self.manifest.commands, &self.subcommands_dir) {
                log::error!("Failed to reload {}: {}", tier.name(), e);
            }
        }
        Ok(())
    }

    pub fn has_commands(&self) -> bool {
        !self.manifest.commands.is_empty()
    }

    fn load_manifest(dir: &Path) -> SubcommandManifest {
        let path = dir.join("manifest.json");
        if !path.exists() {
            return SubcommandManifest::default();
        }
        match fs::read_to_string(&path) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_else(|e| {
                log::warn!("Failed to parse subcommands manifest.json: {}", e);
                SubcommandManifest::default()
            }),
            Err(e) => {
                log::warn!("Failed to read subcommands manifest.json: {}", e);
                SubcommandManifest::default()
            }
        }
    }
}

// --- File I/O for .frames files (shared with wakeword.rs pattern) ---

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
