pub mod piper;

use piper::PiperVoice;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Text-to-speech engine with per-language Piper VITS voices.
///
/// Falls back to macOS `say` for languages without a Piper model (e.g. Korean).
pub struct Tts {
    /// Piper voices keyed by language code ("en", "es", "fr", …)
    voices: HashMap<String, PiperVoice>,
    /// Working directory for macOS `say` temp files
    data_dir: PathBuf,
}

/// Synthesis result: audio samples and their sample rate.
pub struct TtsOutput {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
}

impl Tts {
    /// Create a new TTS engine, loading all available Piper voices from
    /// `data_dir/../models/piper/`.
    pub fn new(data_dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(data_dir)
            .map_err(|e| format!("failed to create TTS data dir: {e}"))?;

        let piper_dir = data_dir
            .parent()
            .unwrap_or(data_dir)
            .join("models")
            .join("piper");

        let voices = load_piper_voices(&piper_dir);

        if voices.is_empty() {
            eprintln!("[tts] no Piper voices found in {}", piper_dir.display());
        } else {
            let langs: Vec<&String> = voices.keys().collect();
            eprintln!("[tts] loaded {} Piper voice(s): {:?}", voices.len(), langs);
        }

        Ok(Self {
            voices,
            data_dir: data_dir.to_path_buf(),
        })
    }

    /// Synthesize text to audio.
    ///
    /// `lang` is a language code like "en", "es", "ko", "zh".
    /// Uses the matching Piper voice if available, otherwise falls back to macOS `say`.
    pub fn synthesize(&mut self, text: &str, lang: &str) -> Result<TtsOutput, String> {
        if text.trim().is_empty() {
            return Ok(TtsOutput {
                samples: Vec::new(),
                sample_rate: 22050,
            });
        }

        // Try Piper voice for this language
        if let Some(voice) = self.voices.get_mut(lang) {
            match voice.synthesize(text) {
                Ok(samples) if !samples.is_empty() => {
                    return Ok(TtsOutput {
                        samples,
                        sample_rate: voice.sample_rate(),
                    });
                }
                Ok(_) => eprintln!("[tts] Piper returned empty audio for lang={lang}, falling back"),
                Err(e) => eprintln!("[tts] Piper failed for lang={lang}: {e}, falling back"),
            }
        }

        // Fallback: macOS `say` with a language-appropriate voice
        let samples = self.synthesize_macos(text, lang)?;
        // macOS say fallback always outputs 24kHz (we convert to that in afconvert)
        Ok(TtsOutput {
            samples,
            sample_rate: 24000,
        })
    }

    /// macOS `say` command: text → AIFF file → WAV → f32 samples at 24kHz.
    /// Uses a language-appropriate voice when available.
    fn synthesize_macos(&self, text: &str, lang: &str) -> Result<Vec<f32>, String> {
        let aiff_path = self.data_dir.join("tts_output.aiff");
        let wav_path = self.data_dir.join("tts_output.wav");

        let mut cmd = Command::new("say");
        cmd.arg("-r").arg("175");
        if let Some(voice) = macos_voice_for_lang(lang) {
            cmd.arg("-v").arg(voice);
        }
        let output = cmd
            .arg("-o")
            .arg(&aiff_path)
            .arg("--")
            .arg(text)
            .output()
            .map_err(|e| format!("failed to run `say`: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "`say` failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        // Convert AIFF to 24kHz f32 WAV
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

        let samples = read_wav_f32(&wav_path)?;

        let _ = std::fs::remove_file(&aiff_path);
        let _ = std::fs::remove_file(&wav_path);

        Ok(samples)
    }
}

// ── Voice loading ───────────────────────────────────────────────────────────

/// Scan a directory for Piper `.onnx` + `.onnx.json` pairs and load them.
/// Returns a map from language code (e.g. "en") to PiperVoice.
fn load_piper_voices(piper_dir: &Path) -> HashMap<String, PiperVoice> {
    let mut voices = HashMap::new();

    let entries = match std::fs::read_dir(piper_dir) {
        Ok(e) => e,
        Err(_) => return voices,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        // Look for .onnx files (not .onnx.json)
        if !name.ends_with(".onnx") || name.ends_with(".onnx.json") {
            continue;
        }

        let config_path = piper_dir.join(format!("{name}.json"));
        if !config_path.exists() {
            eprintln!("[tts] skipping {name}: no companion .json config");
            continue;
        }

        // Extract language family from filename like "en_US-lessac-medium.onnx"
        let lang = match name.split('_').next() {
            Some(l) if !l.is_empty() => l.to_string(),
            _ => continue,
        };

        match PiperVoice::new(&path, &config_path) {
            Ok(voice) => {
                eprintln!(
                    "[tts] loaded Piper voice: {name} (lang={lang}, espeak={}, {}Hz)",
                    voice.espeak_voice(),
                    voice.sample_rate()
                );
                voices.insert(lang, voice);
            }
            Err(e) => {
                eprintln!("[tts] failed to load {name}: {e}");
            }
        }
    }

    voices
}

// ── Language helpers ─────────────────────────────────────────────────────────

/// Map a full language name (as stored in the DB) to a short code.
pub fn language_name_to_code(name: &str) -> &str {
    match name.to_lowercase().as_str() {
        "english" => "en",
        "spanish" | "español" => "es",
        "french" | "français" => "fr",
        "german" | "deutsch" => "de",
        "italian" | "italiano" => "it",
        "korean" | "한국어" => "ko",
        "portuguese" | "português" => "pt",
        "mandarin" | "chinese" | "中文" => "zh",
        "japanese" | "日本語" => "ja",
        "turkish" | "türkçe" => "tr",
        _ => "en",
    }
}

/// Detect language from text using Unicode script analysis.
/// Returns None for Latin-script text (ambiguous between en/es/fr/de/tr).
pub fn detect_language_from_text(text: &str) -> Option<&'static str> {
    for c in text.chars() {
        if matches!(c, '\u{AC00}'..='\u{D7AF}' | '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}')
        {
            return Some("ko");
        }
        if matches!(c, '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}') {
            return Some("zh");
        }
        if matches!(c, '\u{3040}'..='\u{309F}' | '\u{30A0}'..='\u{30FF}') {
            return Some("ja");
        }
    }
    None // Latin script — caller must provide language hint
}

/// macOS `say` voice name for a given language code.
fn macos_voice_for_lang(lang: &str) -> Option<&'static str> {
    match lang {
        "ko" => Some("Yuna"),
        "ja" => Some("Kyoko"),
        "zh" => Some("Ting-Ting"),
        "es" => Some("Monica"),
        "fr" => Some("Thomas"),
        "de" => Some("Anna"),
        "it" => Some("Alice"),
        "pt" => Some("Luciana"),
        "tr" => Some("Yelda"),
        _ => None, // English or unknown — use system default
    }
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
