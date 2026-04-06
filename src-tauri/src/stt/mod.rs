use std::path::Path;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Whisper-based speech-to-text engine.
pub struct WhisperStt {
    ctx: WhisperContext,
}

impl WhisperStt {
    /// Load a Whisper GGML model from disk.
    pub fn new(model_path: &Path) -> Result<Self, String> {
        let params = WhisperContextParameters::default();
        let ctx = WhisperContext::new_with_params(
            model_path.to_str().ok_or("invalid model path")?,
            params,
        )
        .map_err(|e| format!("failed to load whisper model: {e}"))?;

        Ok(Self { ctx })
    }

    /// Transcribe 16kHz mono f32 audio to text in the specified language.
    /// If `lang` is None, Whisper auto-detects the language.
    pub fn transcribe(&self, audio_16k: &[f32], lang: Option<&str>) -> Result<String, String> {
        self.run_whisper(audio_16k, lang, false)
    }

    /// Translate 16kHz mono f32 audio to English text.
    pub fn translate(&self, audio_16k: &[f32]) -> Result<String, String> {
        self.run_whisper(audio_16k, None, true)
    }

    fn run_whisper(
        &self,
        audio_16k: &[f32],
        lang: Option<&str>,
        translate: bool,
    ) -> Result<String, String> {
        let mut state = self
            .ctx
            .create_state()
            .map_err(|e| format!("failed to create whisper state: {e}"))?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_print_special(false);
        params.set_suppress_blank(true);
        params.set_suppress_nst(true);
        params.set_single_segment(false);
        params.set_n_threads(4);
        if let Some(l) = lang {
            params.set_language(Some(l));
        }
        params.set_translate(translate);

        state
            .full(params, audio_16k)
            .map_err(|e| format!("whisper inference failed: {e}"))?;

        let num_segments = state
            .full_n_segments()
            .map_err(|e| format!("failed to get segments: {e}"))?;

        let mut text = String::new();
        for i in 0..num_segments {
            if let Ok(segment_text) = state.full_get_segment_text(i) {
                text.push_str(&segment_text);
            }
        }

        Ok(text.trim().to_string())
    }

    /// Transcribe only the speech segments (from VAD) of a 16kHz buffer.
    /// Falls back to transcribing the full buffer if no segments provided.
    pub fn transcribe_segments(
        &self,
        audio_16k: &[f32],
        segments: &[(usize, usize)],
        lang: Option<&str>,
    ) -> Result<String, String> {
        if segments.is_empty() {
            return self.transcribe(audio_16k, lang);
        }

        let mut full_text = String::new();
        for &(start, end) in segments {
            let start = start.min(audio_16k.len());
            let end = end.min(audio_16k.len());
            if start >= end {
                continue;
            }
            let segment_audio = &audio_16k[start..end];
            // Skip very short segments (< 0.3s)
            if segment_audio.len() < 4800 {
                continue;
            }
            let text = self.transcribe(segment_audio, lang)?;
            if !text.is_empty() {
                if !full_text.is_empty() {
                    full_text.push(' ');
                }
                full_text.push_str(&text);
            }
        }

        Ok(full_text)
    }

    /// Translate only the speech segments (from VAD) to English.
    pub fn translate_segments(
        &self,
        audio_16k: &[f32],
        segments: &[(usize, usize)],
    ) -> Result<String, String> {
        if segments.is_empty() {
            return self.translate(audio_16k);
        }

        let mut full_text = String::new();
        for &(start, end) in segments {
            let start = start.min(audio_16k.len());
            let end = end.min(audio_16k.len());
            if start >= end {
                continue;
            }
            let segment_audio = &audio_16k[start..end];
            if segment_audio.len() < 4800 {
                continue;
            }
            let text = self.translate(segment_audio)?;
            if !text.is_empty() {
                if !full_text.is_empty() {
                    full_text.push(' ');
                }
                full_text.push_str(&text);
            }
        }

        Ok(full_text)
    }
}
