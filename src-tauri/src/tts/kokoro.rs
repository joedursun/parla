use ndarray::{Array1, Array2};
use ort::session::Session;
use ort::value::Tensor;
use std::collections::HashMap;
use std::path::Path;

/// Kokoro 82M TTS engine via ONNX Runtime.
///
/// Input: text → espeak-ng phonemization (via FFI) → token IDs → ONNX inference → 24kHz audio
/// Voice style vectors are loaded from .bin files (raw f32 little-endian).
pub struct KokoroTts {
    session: Session,
    /// Voice style data indexed by padded token count, each entry 256 floats
    voice_data: Vec<f32>,
    /// Phoneme character → token ID mapping
    vocab: HashMap<char, i64>,
}

impl KokoroTts {
    /// Load Kokoro ONNX model and a voice .bin file.
    pub fn new(model_path: &Path, voice_path: &Path) -> Result<Self, String> {
        // Verify espeak-ng phonemization works (initializes the library on first call)
        espeak_rs::text_to_phonemes("test", "en-US", None, true, false)
            .map_err(|e| format!("espeak-ng init failed: {e}"))?;

        let session = Session::builder()
            .map_err(|e| format!("failed to create ONNX session builder: {e}"))?
            .with_intra_threads(2)
            .map_err(|e| format!("failed to set threads: {e}"))?
            .commit_from_file(model_path)
            .map_err(|e| format!("failed to load Kokoro model: {e}"))?;

        let voice_bytes = std::fs::read(voice_path)
            .map_err(|e| format!("failed to read voice file: {e}"))?;
        let voice_data: Vec<f32> = voice_bytes
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();

        if voice_data.len() < 256 {
            return Err(format!(
                "voice file too small: {} bytes, expected at least 1024",
                voice_bytes.len()
            ));
        }

        Ok(Self {
            session,
            voice_data,
            vocab: build_vocab(),
        })
    }

    /// Synthesize text to 24kHz mono f32 audio.
    pub fn synthesize(&mut self, text: &str, speed: f32) -> Result<Vec<f32>, String> {
        if text.trim().is_empty() {
            return Ok(Vec::new());
        }

        // 1. Phonemize via espeak-ng FFI
        let phoneme_sentences =
            espeak_rs::text_to_phonemes(text, "en-US", None, true, false)
                .map_err(|e| format!("phonemization failed: {e}"))?;
        let phonemes = phoneme_sentences.join(" ");
        if phonemes.is_empty() {
            return Ok(Vec::new());
        }

        // 2. Convert phonemes to token IDs
        let token_ids = self.phonemize_to_tokens(&phonemes);
        if token_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Kokoro max context is 510 phoneme tokens
        let token_ids = if token_ids.len() > 510 {
            token_ids[..510].to_vec()
        } else {
            token_ids
        };

        // 3. Pad with 0 at start and end
        let mut padded: Vec<i64> = Vec::with_capacity(token_ids.len() + 2);
        padded.push(0);
        padded.extend_from_slice(&token_ids);
        padded.push(0);

        let n = padded.len();

        // 4. Get voice style vector for this token count
        let style = self.get_style_vector(n)?;

        // 5. Build input tensors
        let input_ids = Array2::from_shape_vec((1, n), padded)
            .map_err(|e| format!("failed to create input_ids tensor: {e}"))?;
        let style_arr = Array2::from_shape_vec((1, 256), style)
            .map_err(|e| format!("failed to create style tensor: {e}"))?;
        let speed_arr = Array1::from_vec(vec![speed]);

        let input_ids_val = Tensor::from_array(input_ids)
            .map_err(|e| format!("failed to create input_ids value: {e}"))?;
        let style_val = Tensor::from_array(style_arr)
            .map_err(|e| format!("failed to create style value: {e}"))?;
        let speed_val = Tensor::from_array(speed_arr)
            .map_err(|e| format!("failed to create speed value: {e}"))?;

        // 6. Run inference
        let outputs = self
            .session
            .run(ort::inputs![
                "input_ids" => input_ids_val,
                "style" => style_val,
                "speed" => speed_val,
            ])
            .map_err(|e| format!("Kokoro inference failed: {e}"))?;

        // 7. Extract audio from output tensor
        let audio_tensor = outputs
            .values()
            .next()
            .ok_or("no output tensor from Kokoro model")?;
        let audio_data = audio_tensor
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("failed to extract audio output: {e}"))?;
        let (_shape, samples) = audio_data;

        Ok(samples.to_vec())
    }

    /// Convert phoneme string to token IDs, dropping unknown chars.
    fn phonemize_to_tokens(&self, phonemes: &str) -> Vec<i64> {
        phonemes
            .chars()
            .filter_map(|c| self.vocab.get(&c).copied())
            .collect()
    }

    /// Get the style vector for a given padded token count.
    fn get_style_vector(&self, token_count: usize) -> Result<Vec<f32>, String> {
        let max_entries = self.voice_data.len() / 256;
        let idx = token_count.min(max_entries.saturating_sub(1));
        let offset = idx * 256;
        if offset + 256 > self.voice_data.len() {
            return Err("voice data too small for this token count".into());
        }
        Ok(self.voice_data[offset..offset + 256].to_vec())
    }
}

/// Build the Kokoro v1.0 phoneme vocabulary (from hexgrad/Kokoro-82M config.json).
fn build_vocab() -> HashMap<char, i64> {
    let mut v = HashMap::new();
    v.insert('$', 0);
    v.insert(';', 1);
    v.insert(':', 2);
    v.insert(',', 3);
    v.insert('.', 4);
    v.insert('!', 5);
    v.insert('?', 6);
    v.insert('\u{2014}', 9);   // em dash —
    v.insert('\u{2026}', 10);  // ellipsis …
    v.insert('"', 11);
    v.insert('(', 12);
    v.insert(')', 13);
    v.insert('\u{201C}', 14);  // left double quotation "
    v.insert('\u{201D}', 15);  // right double quotation "
    v.insert(' ', 16);
    v.insert('\u{0303}', 17);  // combining tilde
    v.insert('\u{02A3}', 18);  // ʣ
    v.insert('\u{02A5}', 19);  // ʥ
    v.insert('\u{02A6}', 20);  // ʦ
    v.insert('\u{02A8}', 21);  // ʨ
    v.insert('\u{1D5D}', 22);  // ᵝ
    v.insert('\u{AB67}', 23);  // ꭧ
    v.insert('A', 24);
    v.insert('I', 25);
    v.insert('O', 31);
    v.insert('Q', 33);
    v.insert('S', 35);
    v.insert('T', 36);
    v.insert('W', 39);
    v.insert('Y', 41);
    v.insert('\u{1D4A}', 42);  // ᵊ
    v.insert('a', 43);
    v.insert('b', 44);
    v.insert('c', 45);
    v.insert('d', 46);
    v.insert('e', 47);
    v.insert('f', 48);
    v.insert('h', 50);
    v.insert('i', 51);
    v.insert('j', 52);
    v.insert('k', 53);
    v.insert('l', 54);
    v.insert('m', 55);
    v.insert('n', 56);
    v.insert('o', 57);
    v.insert('p', 58);
    v.insert('q', 59);
    v.insert('r', 60);
    v.insert('s', 61);
    v.insert('t', 62);
    v.insert('u', 63);
    v.insert('v', 64);
    v.insert('w', 65);
    v.insert('x', 66);
    v.insert('y', 67);
    v.insert('z', 68);
    v.insert('\u{0251}', 69);  // ɑ
    v.insert('\u{0250}', 70);  // ɐ
    v.insert('\u{0252}', 71);  // ɒ
    v.insert('\u{00E6}', 72);  // æ
    v.insert('\u{03B2}', 75);  // β
    v.insert('\u{0254}', 76);  // ɔ
    v.insert('\u{0255}', 77);  // ɕ
    v.insert('\u{00E7}', 78);  // ç
    v.insert('\u{0256}', 80);  // ɖ
    v.insert('\u{00F0}', 81);  // ð
    v.insert('\u{02A4}', 82);  // ʤ
    v.insert('\u{0259}', 83);  // ə
    v.insert('\u{025A}', 85);  // ɚ
    v.insert('\u{025B}', 86);  // ɛ
    v.insert('\u{025C}', 87);  // ɜ
    v.insert('\u{025F}', 90);  // ɟ
    v.insert('\u{0261}', 92);  // ɡ (IPA g)
    v.insert('\u{0265}', 99);  // ɥ
    v.insert('\u{0268}', 101); // ɨ
    v.insert('\u{026A}', 102); // ɪ
    v.insert('\u{029D}', 103); // ʝ
    v.insert('\u{026F}', 110); // ɯ
    v.insert('\u{0270}', 111); // ɰ
    v.insert('\u{014B}', 112); // ŋ
    v.insert('\u{0273}', 113); // ɳ
    v.insert('\u{0272}', 114); // ɲ
    v.insert('\u{0274}', 115); // ɴ
    v.insert('\u{00F8}', 116); // ø
    v.insert('\u{0278}', 118); // ɸ
    v.insert('\u{03B8}', 119); // θ
    v.insert('\u{0153}', 120); // œ
    v.insert('\u{0279}', 123); // ɹ
    v.insert('\u{027E}', 125); // ɾ
    v.insert('\u{027B}', 126); // ɻ
    v.insert('\u{0281}', 128); // ʁ
    v.insert('\u{027D}', 129); // ɽ
    v.insert('\u{0282}', 130); // ʂ
    v.insert('\u{0283}', 131); // ʃ
    v.insert('\u{0288}', 132); // ʈ
    v.insert('\u{02A7}', 133); // ʧ
    v.insert('\u{028A}', 135); // ʊ
    v.insert('\u{028B}', 136); // ʋ
    v.insert('\u{028C}', 138); // ʌ
    v.insert('\u{0263}', 139); // ɣ
    v.insert('\u{0264}', 140); // ɤ
    v.insert('\u{03C7}', 142); // χ
    v.insert('\u{028E}', 143); // ʎ
    v.insert('\u{0292}', 147); // ʒ
    v.insert('\u{0294}', 148); // ʔ
    v.insert('\u{02C8}', 156); // ˈ primary stress
    v.insert('\u{02CC}', 157); // ˌ secondary stress
    v.insert('\u{02D0}', 158); // ː length mark
    v.insert('\u{02B0}', 162); // ʰ aspiration
    v.insert('\u{02B2}', 164); // ʲ palatalization
    v.insert('\u{2193}', 169); // ↓
    v.insert('\u{2192}', 171); // →
    v.insert('\u{2197}', 172); // ↗
    v.insert('\u{2198}', 173); // ↘
    v.insert('\u{1D7B}', 177); // ᵻ
    v
}
