pub mod kokoro;

use kokoro::KokoroTts;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Text-to-speech engine.
/// Uses Kokoro ONNX when available, falls back to macOS built-in synthesis.
pub struct Tts {
    kokoro: Option<KokoroTts>,
    data_dir: PathBuf,
}

impl Tts {
    /// Create a new TTS engine.
    /// Attempts to load Kokoro from `data_dir/../models/`, falls back to macOS `say`.
    pub fn new(data_dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(data_dir)
            .map_err(|e| format!("failed to create TTS data dir: {e}"))?;

        // Try to load Kokoro
        let models_dir = data_dir
            .parent()
            .unwrap_or(data_dir)
            .join("models");

        let kokoro = try_load_kokoro(&models_dir);
        if kokoro.is_some() {
            eprintln!("[tts] Kokoro ONNX loaded successfully");
        } else {
            eprintln!("[tts] Kokoro not available, using macOS say fallback");
        }

        Ok(Self {
            kokoro,
            data_dir: data_dir.to_path_buf(),
        })
    }

    /// Synthesize text to 24kHz mono f32 audio samples.
    pub fn synthesize(&mut self, text: &str) -> Result<Vec<f32>, String> {
        if text.trim().is_empty() {
            return Ok(Vec::new());
        }

        // Try Kokoro first
        if let Some(ref mut kokoro) = self.kokoro {
            match kokoro.synthesize(text, 1.0) {
                Ok(samples) if !samples.is_empty() => return Ok(samples),
                Ok(_) => eprintln!("[tts] Kokoro returned empty audio, falling back"),
                Err(e) => eprintln!("[tts] Kokoro failed: {e}, falling back"),
            }
        }

        // Fallback to macOS say
        self.synthesize_macos(text)
    }

    /// macOS `say` command: text → AIFF file → WAV → f32 samples at 24kHz
    fn synthesize_macos(&self, text: &str) -> Result<Vec<f32>, String> {
        let aiff_path = self.data_dir.join("tts_output.aiff");
        let wav_path = self.data_dir.join("tts_output.wav");

        // Generate speech with `say`
        let output = Command::new("say")
            .arg("-r").arg("175")
            .arg("-o").arg(&aiff_path)
            .arg("--").arg(text)
            .output()
            .map_err(|e| format!("failed to run `say`: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "`say` failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        // Convert AIFF to WAV at 24kHz using afconvert (built-in macOS tool)
        let output = Command::new("afconvert")
            .args(["-f", "WAVE", "-d", "LEF32@24000"])
            .arg(&aiff_path)
            .arg(&wav_path)
            .output()
            .map_err(|e| format!("failed to run `afconvert`: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "`afconvert` failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        // Read WAV file and extract f32 samples
        let samples = read_wav_f32(&wav_path)?;

        // Clean up temp files
        let _ = std::fs::remove_file(&aiff_path);
        let _ = std::fs::remove_file(&wav_path);

        Ok(samples)
    }
}

/// Try to load Kokoro model and a voice file.
fn try_load_kokoro(models_dir: &Path) -> Option<KokoroTts> {
    let model_path = models_dir.join("kokoro-v0_19.onnx");
    if !model_path.exists() {
        // Also try v1.0 naming
        let alt = models_dir.join("kokoro.onnx");
        if !alt.exists() {
            return None;
        }
        return try_load_kokoro_with_model(&alt, models_dir);
    }
    try_load_kokoro_with_model(&model_path, models_dir)
}

fn try_load_kokoro_with_model(model_path: &Path, models_dir: &Path) -> Option<KokoroTts> {
    // Look for any voice .bin file in models/voices/
    let voices_dir = models_dir.join("voices");
    let voice_path = find_voice_file(&voices_dir)
        .or_else(|| {
            // Also check directly in models dir
            find_voice_file(models_dir)
        })?;

    match KokoroTts::new(model_path, &voice_path) {
        Ok(k) => Some(k),
        Err(e) => {
            eprintln!("[tts] failed to load Kokoro: {e}");
            None
        }
    }
}

fn find_voice_file(dir: &Path) -> Option<PathBuf> {
    // Prefer af_heart.bin (a popular default voice)
    let preferred = dir.join("af_heart.bin");
    if preferred.exists() {
        return Some(preferred);
    }
    // Find any .bin file
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            if name.to_string_lossy().ends_with(".bin") {
                return Some(entry.path());
            }
        }
    }
    None
}

/// Read a WAV file with f32 samples.
fn read_wav_f32(path: &Path) -> Result<Vec<f32>, String> {
    let reader =
        hound::WavReader::open(path).map_err(|e| format!("failed to open WAV file: {e}"))?;

    let spec = reader.spec();

    match spec.sample_format {
        hound::SampleFormat::Float => {
            let samples: Vec<f32> = reader
                .into_samples::<f32>()
                .filter_map(|s| s.ok())
                .collect();
            Ok(samples)
        }
        hound::SampleFormat::Int => {
            let bits = spec.bits_per_sample;
            let max_val = (1 << (bits - 1)) as f32;
            let samples: Vec<f32> = reader
                .into_samples::<i32>()
                .filter_map(|s| s.ok())
                .map(|s| s as f32 / max_val)
                .collect();
            Ok(samples)
        }
    }
}
