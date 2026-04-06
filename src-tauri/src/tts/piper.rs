use ndarray::{Array1, Array2};
use ort::session::Session;
use ort::value::Tensor;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

// ── Config types (deserialized from .onnx.json) ─────────────────────────────

#[derive(Deserialize)]
struct PiperConfig {
    audio: AudioConfig,
    espeak: EspeakConfig,
    inference: InferenceConfig,
    phoneme_id_map: HashMap<String, Vec<i64>>,
    #[serde(default)]
    phoneme_map: HashMap<String, String>,
    #[serde(default = "default_num_speakers")]
    num_speakers: u32,
}

fn default_num_speakers() -> u32 {
    1
}

#[derive(Deserialize)]
struct AudioConfig {
    sample_rate: u32,
}

#[derive(Deserialize)]
struct EspeakConfig {
    voice: String,
}

#[derive(Deserialize)]
struct InferenceConfig {
    noise_scale: f32,
    length_scale: f32,
    noise_w: f32,
}

// ── PiperVoice ──────────────────────────────────────────────────────────────

/// A single Piper VITS voice model loaded from ONNX.
///
/// Pipeline: text → espeak-ng phonemization → phoneme IDs → ONNX inference → audio
pub struct PiperVoice {
    session: Session,
    phoneme_id_map: HashMap<String, Vec<i64>>,
    phoneme_map: HashMap<String, String>,
    espeak_voice: String,
    sample_rate: u32,
    noise_scale: f32,
    length_scale: f32,
    noise_w: f32,
    num_speakers: u32,
}

impl PiperVoice {
    /// Load a Piper voice from an ONNX model and its companion JSON config.
    pub fn new(model_path: &Path, config_path: &Path) -> Result<Self, String> {
        let config_str = std::fs::read_to_string(config_path)
            .map_err(|e| format!("failed to read Piper config {}: {e}", config_path.display()))?;
        let config: PiperConfig = serde_json::from_str(&config_str)
            .map_err(|e| format!("failed to parse Piper config: {e}"))?;

        let session = Session::builder()
            .map_err(|e| format!("ONNX session builder: {e}"))?
            .with_intra_threads(2)
            .map_err(|e| format!("set threads: {e}"))?
            .commit_from_file(model_path)
            .map_err(|e| format!("load Piper model {}: {e}", model_path.display()))?;

        Ok(Self {
            session,
            phoneme_id_map: config.phoneme_id_map,
            phoneme_map: config.phoneme_map,
            espeak_voice: config.espeak.voice,
            sample_rate: config.audio.sample_rate,
            noise_scale: config.inference.noise_scale,
            length_scale: config.inference.length_scale,
            noise_w: config.inference.noise_w,
            num_speakers: config.num_speakers,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn espeak_voice(&self) -> &str {
        &self.espeak_voice
    }

    /// Synthesize text to mono f32 audio at this voice's native sample rate.
    pub fn synthesize(&mut self, text: &str) -> Result<Vec<f32>, String> {
        if text.trim().is_empty() {
            return Ok(Vec::new());
        }

        // 1. Phonemize via espeak-ng FFI
        let phoneme_sentences =
            espeak_rs::text_to_phonemes(text, &self.espeak_voice, None, true, false)
                .map_err(|e| format!("phonemization failed: {e}"))?;
        let phonemes = phoneme_sentences.join(" ");
        if phonemes.is_empty() {
            return Ok(Vec::new());
        }

        // 2. Apply phoneme_map substitutions (usually empty, but some voices use it)
        let phonemes = if self.phoneme_map.is_empty() {
            phonemes
        } else {
            let mut result = phonemes;
            for (from, to) in &self.phoneme_map {
                result = result.replace(from.as_str(), to.as_str());
            }
            result
        };

        // 3. Convert phonemes to token IDs (VITS intersperse-blank pattern)
        let ids = self.phonemes_to_ids(&phonemes);
        if ids.len() <= 2 {
            // Only BOS + EOS, no real content
            return Ok(Vec::new());
        }

        let seq_len = ids.len();

        // 4. Build input tensors
        let input = Array2::from_shape_vec((1, seq_len), ids)
            .map_err(|e| format!("input tensor: {e}"))?;
        let input_lengths = Array1::from_vec(vec![seq_len as i64]);
        let scales = Array1::from_vec(vec![self.noise_scale, self.length_scale, self.noise_w]);

        let input_val =
            Tensor::from_array(input).map_err(|e| format!("input value: {e}"))?;
        let lengths_val =
            Tensor::from_array(input_lengths).map_err(|e| format!("lengths value: {e}"))?;
        let scales_val =
            Tensor::from_array(scales).map_err(|e| format!("scales value: {e}"))?;

        // 5. Run ONNX inference
        let outputs = if self.num_speakers > 1 {
            let sid = Array1::from_vec(vec![0i64]);
            let sid_val = Tensor::from_array(sid).map_err(|e| format!("sid value: {e}"))?;
            self.session
                .run(ort::inputs![
                    "input" => input_val,
                    "input_lengths" => lengths_val,
                    "scales" => scales_val,
                    "sid" => sid_val,
                ])
                .map_err(|e| format!("Piper inference failed: {e}"))?
        } else {
            self.session
                .run(ort::inputs![
                    "input" => input_val,
                    "input_lengths" => lengths_val,
                    "scales" => scales_val,
                ])
                .map_err(|e| format!("Piper inference failed: {e}"))?
        };

        // 6. Extract audio from output tensor (shape [1, 1, N])
        let audio_tensor = outputs.values().next().ok_or("no output tensor")?;
        let (_shape, samples) = audio_tensor
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("extract audio: {e}"))?;

        Ok(samples.to_vec())
    }

    /// Convert a phoneme string to Piper token IDs.
    ///
    /// Uses the VITS "intersperse blank" pattern:
    ///   BOS, phoneme₁, PAD, phoneme₂, PAD, …, EOS
    fn phonemes_to_ids(&self, phonemes: &str) -> Vec<i64> {
        let bos = self.lookup("^").unwrap_or(vec![1]);
        let eos = self.lookup("$").unwrap_or(vec![2]);
        let pad = self.lookup("_").unwrap_or(vec![0]);

        let mut ids = Vec::with_capacity(phonemes.len() * 3 + 4);
        ids.extend(&bos);

        for c in phonemes.chars() {
            let key = c.to_string();
            if let Some(mapped) = self.phoneme_id_map.get(&key) {
                ids.extend(mapped);
                ids.extend(&pad);
            }
            // Unknown phonemes are silently skipped (matches Piper C++ behaviour)
        }

        ids.extend(&eos);
        ids
    }

    fn lookup(&self, key: &str) -> Option<Vec<i64>> {
        self.phoneme_id_map.get(key).cloned()
    }
}
