use std::path::PathBuf;
use std::sync::mpsc as std_mpsc;
use std::thread;
use tokio::sync::mpsc;

use super::capture::{AudioCapture, AudioChunk};
use super::playback::AudioPlayback;
use crate::vad::{SileroVad, SpeechSegment};

enum AudioCommand {
    Init {
        reply: std_mpsc::Sender<Result<(), String>>,
    },
    InitVad {
        model_path: PathBuf,
        reply: std_mpsc::Sender<Result<(), String>>,
    },
    StartRecording {
        reply: std_mpsc::Sender<Result<(), String>>,
    },
    StopRecording {
        reply: std_mpsc::Sender<Result<RecordingResult, String>>,
    },
    PlayAudio {
        samples: Vec<f32>,
        sample_rate: u32,
        reply: std_mpsc::Sender<Result<(), String>>,
    },
    StopPlayback,
    Status {
        reply: std_mpsc::Sender<AudioPipelineStatus>,
    },
}

pub struct RecordingResult {
    pub samples: Vec<f32>,
    pub speech_segments: Vec<SpeechSegment>,
}

pub struct AudioPipelineStatus {
    pub is_recording: bool,
    pub is_playing: bool,
    pub vad_active: bool,
    pub speech_detected: bool,
}

pub struct AudioState {
    cmd_tx: std_mpsc::Sender<AudioCommand>,
}

impl AudioState {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = std_mpsc::channel::<AudioCommand>();
        thread::spawn(move || audio_thread_main(cmd_rx));
        Self { cmd_tx }
    }

    pub fn init(&self) -> Result<(), String> {
        let (reply_tx, reply_rx) = std_mpsc::channel();
        self.cmd_tx
            .send(AudioCommand::Init { reply: reply_tx })
            .map_err(|_| "audio thread gone".to_string())?;
        reply_rx
            .recv()
            .map_err(|_| "audio thread gone".to_string())?
    }

    pub fn init_vad(&self, model_path: PathBuf) -> Result<(), String> {
        let (reply_tx, reply_rx) = std_mpsc::channel();
        self.cmd_tx
            .send(AudioCommand::InitVad {
                model_path,
                reply: reply_tx,
            })
            .map_err(|_| "audio thread gone".to_string())?;
        reply_rx
            .recv()
            .map_err(|_| "audio thread gone".to_string())?
    }

    pub fn start_recording(&self) -> Result<(), String> {
        let (reply_tx, reply_rx) = std_mpsc::channel();
        self.cmd_tx
            .send(AudioCommand::StartRecording { reply: reply_tx })
            .map_err(|_| "audio thread gone".to_string())?;
        reply_rx
            .recv()
            .map_err(|_| "audio thread gone".to_string())?
    }

    pub fn stop_recording(&self) -> Result<RecordingResult, String> {
        let (reply_tx, reply_rx) = std_mpsc::channel();
        self.cmd_tx
            .send(AudioCommand::StopRecording { reply: reply_tx })
            .map_err(|_| "audio thread gone".to_string())?;
        reply_rx
            .recv()
            .map_err(|_| "audio thread gone".to_string())?
    }

    pub fn play_audio(&self, samples: Vec<f32>, sample_rate: u32) -> Result<(), String> {
        let (reply_tx, reply_rx) = std_mpsc::channel();
        self.cmd_tx
            .send(AudioCommand::PlayAudio {
                samples,
                sample_rate,
                reply: reply_tx,
            })
            .map_err(|_| "audio thread gone".to_string())?;
        reply_rx
            .recv()
            .map_err(|_| "audio thread gone".to_string())?
    }

    pub fn stop_playback(&self) {
        let _ = self.cmd_tx.send(AudioCommand::StopPlayback);
    }

    pub fn status(&self) -> AudioPipelineStatus {
        let (reply_tx, reply_rx) = std_mpsc::channel();
        let default = AudioPipelineStatus {
            is_recording: false,
            is_playing: false,
            vad_active: false,
            speech_detected: false,
        };
        if self
            .cmd_tx
            .send(AudioCommand::Status { reply: reply_tx })
            .is_ok()
        {
            reply_rx.recv().unwrap_or(default)
        } else {
            default
        }
    }
}

/// State held by the audio thread.
struct AudioThreadState {
    capture: Option<AudioCapture>,
    playback: Option<AudioPlayback>,
    capture_rx: Option<mpsc::UnboundedReceiver<AudioChunk>>,
    /// Raw recording buffer at the device's native sample rate.
    raw_buffer: Vec<f32>,
    /// Device capture sample rate (set after init).
    capture_rate: u32,
    vad: Option<SileroVad>,
    speech_segments: Vec<SpeechSegment>,
}

impl AudioThreadState {
    fn new() -> Self {
        Self {
            capture: None,
            playback: None,
            capture_rx: None,
            raw_buffer: Vec::new(),
            capture_rate: 16000,
            vad: None,
            speech_segments: Vec::new(),
        }
    }

    /// Drain all pending audio chunks from the capture channel into raw_buffer.
    fn drain_capture(&mut self) {
        if let Some(ref mut rx) = self.capture_rx {
            while let Ok(chunk) = rx.try_recv() {
                self.raw_buffer.extend_from_slice(&chunk.samples);
            }
        }
    }

    /// Resample the raw buffer from the device rate to 16kHz.
    /// Returns the 16kHz mono audio.
    fn resample_to_16k(&self) -> Vec<f32> {
        if self.capture_rate == 16000 {
            self.raw_buffer.clone()
        } else {
            super::resampler::resample_mono(&self.raw_buffer, self.capture_rate, 16000)
        }
    }

    /// Run VAD over a 16kHz buffer and collect speech segments.
    fn run_vad(&mut self, audio_16k: &[f32]) {
        if let Some(ref mut v) = self.vad {
            for chunk in audio_16k.chunks(512) {
                if chunk.len() == 512 {
                    if let Ok(segs) = v.process_chunk(chunk) {
                        self.speech_segments.extend(segs);
                    }
                }
            }
        }
    }
}

fn audio_thread_main(cmd_rx: std_mpsc::Receiver<AudioCommand>) {
    let mut s = AudioThreadState::new();

    while let Ok(cmd) = cmd_rx.recv() {
        match cmd {
            AudioCommand::Init { reply } => {
                let result = (|| -> Result<(), String> {
                    let (tx, rx) = mpsc::unbounded_channel();
                    let capture = AudioCapture::new(tx)?;
                    s.capture_rate = capture.device_rate();
                    s.capture = Some(capture);
                    s.playback = Some(AudioPlayback::new()?);
                    s.capture_rx = Some(rx);
                    Ok(())
                })();
                let _ = reply.send(result);
            }
            AudioCommand::InitVad { model_path, reply } => {
                let result = SileroVad::new(&model_path, 0.5).map(|v| {
                    s.vad = Some(v);
                });
                let _ = reply.send(result);
            }
            AudioCommand::StartRecording { reply } => {
                s.raw_buffer.clear();
                s.speech_segments.clear();
                if let Some(ref mut v) = s.vad {
                    v.reset();
                }
                let result = if let Some(ref cap) = s.capture {
                    cap.start()
                } else {
                    Err("audio not initialized".into())
                };
                let _ = reply.send(result);
            }
            AudioCommand::StopRecording { reply } => {
                if let Some(ref cap) = s.capture {
                    cap.stop();
                }
                // Small delay to let the last callback chunks arrive
                std::thread::sleep(std::time::Duration::from_millis(50));
                s.drain_capture();

                // Batch-resample the entire recording to 16kHz
                let audio_16k = s.resample_to_16k();
                s.run_vad(&audio_16k);

                let _ = reply.send(Ok(RecordingResult {
                    samples: audio_16k,
                    speech_segments: std::mem::take(&mut s.speech_segments),
                }));
                s.raw_buffer.clear();
            }
            AudioCommand::PlayAudio {
                samples,
                sample_rate,
                reply,
            } => {
                let result = if let Some(ref mut pb) = s.playback {
                    pb.queue_audio(&samples, sample_rate);
                    Ok(())
                } else {
                    Err("audio not initialized".into())
                };
                let _ = reply.send(result);
            }
            AudioCommand::StopPlayback => {
                if let Some(ref mut pb) = s.playback {
                    pb.flush();
                }
            }
            AudioCommand::Status { reply } => {
                s.drain_capture();
                let _ = reply.send(AudioPipelineStatus {
                    is_recording: s.capture.as_ref().map_or(false, |c| c.is_recording()),
                    is_playing: s.playback.as_ref().map_or(false, |p| p.is_playing()),
                    vad_active: s.vad.is_some(),
                    speech_detected: s.vad.as_ref().map_or(false, |v| v.is_speaking()),
                });
            }
        }
    }
}
