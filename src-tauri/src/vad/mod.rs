use ndarray::{Array1, Array2, Array3};
use ort::session::Session;
use ort::value::Tensor;
use std::path::Path;

/// Silero VAD v5 wrapper.
/// Processes 16kHz mono audio in chunks and detects speech segments.
pub struct SileroVad {
    session: Session,
    /// Hidden state: (2, 1, 64) for v5
    h: Array3<f32>,
    c: Array3<f32>,
    /// Sample rate as a 1-element i64 tensor
    sr: Array1<i64>,
    /// Activation threshold
    threshold: f32,
    /// Minimum silence duration in samples before ending a speech segment
    min_silence_samples: usize,
    /// Padding to add before detected speech start (in samples)
    speech_pad_samples: usize,
    /// State tracking
    triggered: bool,
    /// Current position within the audio stream (in samples)
    current_sample: usize,
    /// When speech was last detected as starting
    speech_start_sample: Option<usize>,
    /// Counter of silence samples since last speech
    silence_samples: usize,
}

/// A detected speech segment.
pub struct SpeechSegment {
    pub start_sample: usize,
    pub end_sample: usize,
}

impl SileroVad {
    /// Load the Silero VAD ONNX model from the given path.
    pub fn new(model_path: &Path, threshold: f32) -> Result<Self, String> {
        let session = Session::builder()
            .map_err(|e| format!("failed to create session builder: {e}"))?
            .with_intra_threads(1)
            .map_err(|e| format!("failed to set threads: {e}"))?
            .commit_from_file(model_path)
            .map_err(|e| format!("failed to load VAD model: {e}"))?;

        let h = Array3::<f32>::zeros((2, 1, 64));
        let c = Array3::<f32>::zeros((2, 1, 64));
        let sr = Array1::<i64>::from_elem(1, 16000);

        Ok(Self {
            session,
            h,
            c,
            sr,
            threshold,
            min_silence_samples: (0.55 * 16000.0) as usize,
            speech_pad_samples: (0.5 * 16000.0) as usize,
            triggered: false,
            current_sample: 0,
            speech_start_sample: None,
            silence_samples: 0,
        })
    }

    /// Process a chunk of 16kHz mono audio (typically 512 samples = 32ms).
    /// Returns any completed speech segments.
    pub fn process_chunk(&mut self, audio: &[f32]) -> Result<Vec<SpeechSegment>, String> {
        let chunk_len = audio.len();

        let input_tensor = Array2::from_shape_vec((1, chunk_len), audio.to_vec())
            .map_err(|e| format!("failed to create input tensor: {e}"))?;

        let input_val = Tensor::from_array(input_tensor)
            .map_err(|e| format!("failed to create tensor: {e}"))?;
        let sr_val = Tensor::from_array(self.sr.clone())
            .map_err(|e| format!("failed to create sr tensor: {e}"))?;
        let h_val = Tensor::from_array(self.h.clone())
            .map_err(|e| format!("failed to create h tensor: {e}"))?;
        let c_val = Tensor::from_array(self.c.clone())
            .map_err(|e| format!("failed to create c tensor: {e}"))?;

        let outputs = self
            .session
            .run(ort::inputs![
                "input" => input_val,
                "sr" => sr_val,
                "h" => h_val,
                "c" => c_val,
            ])
            .map_err(|e| format!("VAD inference failed: {e}"))?;

        // Extract speech probability
        let prob = outputs["output"]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("failed to extract output: {e}"))?;
        let (_shape, prob_data) = prob;
        let speech_prob = prob_data.first().copied().unwrap_or(0.0);

        // Update hidden states
        if let Ok(hn) = outputs["hn"].try_extract_tensor::<f32>() {
            let (_shape, slice) = hn;
            let expected = 2 * 1 * 64;
            if slice.len() == expected {
                self.h = Array3::from_shape_vec((2, 1, 64), slice.to_vec())
                    .unwrap_or_else(|_| Array3::zeros((2, 1, 64)));
            }
        }
        if let Ok(cn) = outputs["cn"].try_extract_tensor::<f32>() {
            let (_shape, slice) = cn;
            let expected = 2 * 1 * 64;
            if slice.len() == expected {
                self.c = Array3::from_shape_vec((2, 1, 64), slice.to_vec())
                    .unwrap_or_else(|_| Array3::zeros((2, 1, 64)));
            }
        }

        let mut segments = Vec::new();

        if speech_prob >= self.threshold {
            self.silence_samples = 0;
            if !self.triggered {
                self.triggered = true;
                let start = self.current_sample.saturating_sub(self.speech_pad_samples);
                self.speech_start_sample = Some(start);
            }
        } else if self.triggered {
            self.silence_samples += chunk_len;
            if self.silence_samples >= self.min_silence_samples {
                let end = self.current_sample + chunk_len + self.speech_pad_samples;
                if let Some(start) = self.speech_start_sample.take() {
                    segments.push(SpeechSegment {
                        start_sample: start,
                        end_sample: end,
                    });
                }
                self.triggered = false;
                self.silence_samples = 0;
            }
        }

        self.current_sample += chunk_len;

        Ok(segments)
    }

    /// Reset the VAD state.
    pub fn reset(&mut self) {
        self.h = Array3::zeros((2, 1, 64));
        self.c = Array3::zeros((2, 1, 64));
        self.triggered = false;
        self.current_sample = 0;
        self.speech_start_sample = None;
        self.silence_samples = 0;
    }

    /// Check if speech is currently being detected.
    pub fn is_speaking(&self) -> bool {
        self.triggered
    }
}
