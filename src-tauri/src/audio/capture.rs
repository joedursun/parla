use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, SampleRate, Stream, StreamConfig};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Audio samples captured from the microphone as mono f32 at the device's native rate.
pub struct AudioChunk {
    pub samples: Vec<f32>,
}

/// Manages microphone capture. Runs on a dedicated cpal audio thread
/// and sends chunks to an async channel.
pub struct AudioCapture {
    stream: Option<Stream>,
    is_recording: Arc<AtomicBool>,
    device_rate: u32,
}

impl AudioCapture {
    pub fn new(tx: mpsc::UnboundedSender<AudioChunk>) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("no input device available")?;

        let default_config = device
            .default_input_config()
            .map_err(|e| format!("failed to get default input config: {e}"))?;

        let native_rate = default_config.sample_rate().0;
        let native_channels = default_config.channels();
        let sample_format = default_config.sample_format();

        let config = StreamConfig {
            channels: native_channels,
            sample_rate: SampleRate(native_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        let channels = native_channels as usize;
        let is_recording = Arc::new(AtomicBool::new(false));
        let recording_flag = is_recording.clone();

        let stream = match sample_format {
            SampleFormat::F32 => device.build_input_stream(
                &config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if !recording_flag.load(Ordering::Relaxed) {
                        return;
                    }
                    let mono = if channels > 1 {
                        data.chunks(channels)
                            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                            .collect::<Vec<f32>>()
                    } else {
                        data.to_vec()
                    };
                    if !mono.is_empty() {
                        let _ = tx.send(AudioChunk { samples: mono });
                    }
                },
                |err| eprintln!("audio capture error: {err}"),
                None,
            ),
            SampleFormat::I16 => device.build_input_stream(
                &config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    if !recording_flag.load(Ordering::Relaxed) {
                        return;
                    }
                    let f32_data: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
                    let mono = if channels > 1 {
                        f32_data
                            .chunks(channels)
                            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                            .collect::<Vec<f32>>()
                    } else {
                        f32_data
                    };
                    if !mono.is_empty() {
                        let _ = tx.send(AudioChunk { samples: mono });
                    }
                },
                |err| eprintln!("audio capture error: {err}"),
                None,
            ),
            _ => return Err(format!("unsupported sample format: {sample_format:?}")),
        }
        .map_err(|e| format!("failed to build input stream: {e}"))?;

        Ok(Self {
            stream: Some(stream),
            is_recording,
            device_rate: native_rate,
        })
    }

    pub fn start(&self) -> Result<(), String> {
        if let Some(ref stream) = self.stream {
            stream
                .play()
                .map_err(|e| format!("failed to start stream: {e}"))?;
        }
        self.is_recording.store(true, Ordering::Relaxed);
        Ok(())
    }

    pub fn stop(&self) {
        self.is_recording.store(false, Ordering::Relaxed);
    }

    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::Relaxed)
    }

    /// The sample rate of the captured audio (device native rate).
    pub fn device_rate(&self) -> u32 {
        self.device_rate
    }
}
