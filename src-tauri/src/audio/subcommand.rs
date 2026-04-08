//! Subcommand cascade: tiered recognition for voice commands after "Крео, приём".
//!
//! Architecture:
//! - Tier 1 (DTW): Embedding-level matching for fixed subcommands (<50ms)
//! - Tier 2 (Vosk): Grammar-constrained STT for text subcommands (<100ms)
//! - Tier 3 (Qwen3): LLM + GBNF for parametric commands (0.5-2s) — future

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::embedding::{
    dtw_normalized_distance, load_frames_file, EmbeddingExtractor, FrameSequence,
    DTW_DISTANCE_THRESHOLD, MIN_DETECTION_FRAMES,
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

// --- Tier 2: Vosk grammar-constrained STT ---

#[cfg(feature = "vosk")]
pub struct VoskTier {
    model: vosk::Model,
    /// Recognized phrase (lowercase) → (command_name, action)
    phrase_map: HashMap<String, (String, String)>,
    /// Grammar phrases for recognizer creation (includes "[unk]")
    grammar: Vec<String>,
}

#[cfg(feature = "vosk")]
impl VoskTier {
    pub fn new(model_path: &str, manifest: &SubcommandManifest) -> Result<Self> {
        let model = vosk::Model::new(model_path)
            .ok_or_else(|| anyhow::anyhow!("Failed to load Vosk model from {}", model_path))?;

        let mut tier = Self {
            model,
            phrase_map: HashMap::new(),
            grammar: Vec::new(),
        };
        tier.build_grammar(manifest);
        Ok(tier)
    }

    fn build_grammar(&mut self, manifest: &SubcommandManifest) {
        self.phrase_map.clear();
        self.grammar.clear();

        for cmd in &manifest.commands {
            if cmd.tier != SubcommandTierKind::Vosk {
                continue;
            }
            for phrase in &cmd.phrases {
                let lower = phrase.to_lowercase();
                if let Some(old) = self.phrase_map.insert(
                    lower.clone(),
                    (cmd.name.clone(), cmd.action.clone()),
                ) {
                    log::warn!(
                        "VoskTier: phrase '{}' duplicated, overwriting command '{}'",
                        lower,
                        old.0
                    );
                }
                self.grammar.push(lower);
            }
        }

        // [unk] enables rejection of unrecognized speech
        self.grammar.push("[unk]".to_string());

        log::info!(
            "VoskTier: {} phrases across {} commands",
            self.phrase_map.len(),
            manifest
                .commands
                .iter()
                .filter(|c| c.tier == SubcommandTierKind::Vosk)
                .count()
        );
    }
}

#[cfg(feature = "vosk")]
impl SubcommandTier for VoskTier {
    fn try_match(&mut self, audio: &[f32]) -> Option<SubcommandMatch> {
        if self.phrase_map.is_empty() {
            return None;
        }

        // Convert f32 [-1.0, 1.0] to i16 for Vosk
        let samples: Vec<i16> = audio
            .iter()
            .map(|&s| (s * 32767.0).clamp(-32768.0, 32767.0) as i16)
            .collect();

        let grammar_refs: Vec<&str> = self.grammar.iter().map(|s| s.as_str()).collect();
        let mut recognizer =
            match vosk::Recognizer::new_with_grammar(&self.model, 16000.0, &grammar_refs) {
                Some(r) => r,
                None => {
                    log::error!("VoskTier: failed to create recognizer");
                    return None;
                }
            };

        recognizer.set_words(true);

        // Feed entire audio buffer (short utterance, already VAD-segmented).
        // Recognizer is created per call: grammar can change on reload(),
        // and per-call cost is negligible for short utterances.
        if let Err(e) = recognizer.accept_waveform(&samples) {
            log::error!("VoskTier: accept_waveform error: {}", e);
            return None;
        }
        let result = recognizer.final_result();

        let single = match &result {
            vosk::CompleteResult::Single(s) => s,
            vosk::CompleteResult::Multiple(m) => {
                // Shouldn't happen with max_alternatives=0 (default), but handle gracefully
                if let Some(alt) = m.alternatives.first() {
                    log::info!(
                        "VoskTier: multiple result, using first: '{}' conf={:.4}",
                        alt.text,
                        alt.confidence
                    );
                    // Fall through to phrase lookup below with the text
                    let text = alt.text.trim().to_lowercase();
                    if text == "[unk]" || text.is_empty() {
                        log::info!("VoskTier: [unk] or empty → reject");
                        return None;
                    }
                    if let Some((name, action)) = self.phrase_map.get(&text) {
                        return Some(SubcommandMatch {
                            command_name: name.clone(),
                            action: action.clone(),
                            confidence: alt.confidence,
                            tier: 2,
                            params: HashMap::new(),
                        });
                    }
                }
                return None;
            }
        };

        let text = single.text.trim().to_lowercase();

        log::info!(
            "VoskTier: recognized '{}' (words: {})",
            text,
            single.result.len()
        );

        // Reject [unk] or empty
        if text == "[unk]" || text.is_empty() {
            log::info!("VoskTier: [unk] or empty → reject");
            return None;
        }

        // Look up command by phrase
        if let Some((name, action)) = self.phrase_map.get(&text) {
            let confidence = if single.result.is_empty() {
                0.5
            } else {
                single.result.iter().map(|w| w.conf).sum::<f32>() / single.result.len() as f32
            };

            log::info!(
                "  VoskTier '{}' → command='{}' action='{}' confidence={:.4} MATCH",
                text,
                name,
                action,
                confidence,
            );

            Some(SubcommandMatch {
                command_name: name.clone(),
                action: action.clone(),
                confidence,
                tier: 2,
                params: HashMap::new(),
            })
        } else {
            log::info!("  VoskTier '{}' → no matching command, reject", text);
            None
        }
    }

    fn reload(&mut self, commands: &[SubcommandDef], _subcommands_dir: &Path) -> Result<()> {
        let manifest = SubcommandManifest {
            commands: commands.to_vec(),
        };
        self.build_grammar(&manifest);
        Ok(())
    }

    fn name(&self) -> &str {
        "Vosk"
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
        vosk_model_path: Option<&str>,
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
        let mut tiers: Vec<Box<dyn SubcommandTier>> = vec![Box::new(dtw_tier)];

        // Tier 2: Vosk grammar-constrained STT (optional)
        #[cfg(feature = "vosk")]
        if let Some(vosk_path) = vosk_model_path {
            if std::path::Path::new(vosk_path).exists() {
                match VoskTier::new(vosk_path, &manifest) {
                    Ok(tier) => {
                        log::info!("VoskTier loaded from {}", vosk_path);
                        tiers.push(Box::new(tier));
                    }
                    Err(e) => {
                        log::warn!("VoskTier not available: {}", e);
                    }
                }
            } else {
                log::info!("Vosk model not found at {}, skipping Tier 2", vosk_path);
            }
        }

        #[cfg(not(feature = "vosk"))]
        let _ = vosk_model_path; // suppress unused warning

        log::info!(
            "SubcommandCascade loaded: {} DTW commands, {} tiers total",
            dtw_count,
            tiers.len(),
        );

        Ok(Self {
            tiers,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Pinned wire format for SubcommandTierKind — persisted in manifest.json.
    /// If this fails, you changed a serialized value. Update ONLY if intentional
    /// AND you've handled migration of existing manifest files.
    #[test]
    fn subcommand_tier_kind_serialization_stability() {
        assert_eq!(
            serde_json::to_string(&SubcommandTierKind::Dtw).unwrap(),
            "\"dtw\""
        );
        assert_eq!(
            serde_json::to_string(&SubcommandTierKind::Vosk).unwrap(),
            "\"vosk\""
        );
        assert_eq!(
            serde_json::to_string(&SubcommandTierKind::Llm).unwrap(),
            "\"llm\""
        );
    }
}

