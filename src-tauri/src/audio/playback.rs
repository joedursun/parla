use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, SampleRate, Stream, StreamConfig};
use ringbuf::{HeapRb, traits::{Consumer, Producer, Split}};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const PLAYBACK_RING_SIZE: usize = 48000 * 10; // 10 seconds at 48kHz

/// Manages audio playback through the default output device.
/// Audio is fed into a ring buffer and played back continuously.
pub struct AudioPlayback {
    stream: Stream,
    producer: ringbuf::HeapProd<f32>,
    is_playing: Arc<AtomicBool>,
    device_rate: u32,
}

impl AudioPlayback {
    pub fn new() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or("no output device available")?;

        let default_config = device
            .default_output_config()
            .map_err(|e| format!("failed to get default output config: {e}"))?;

        let device_rate = default_config.sample_rate().0;
        let device_channels = default_config.channels();
        let sample_format = default_config.sample_format();

        let config = StreamConfig {
            channels: device_channels,
            sample_rate: SampleRate(device_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        let rb = HeapRb::<f32>::new(PLAYBACK_RING_SIZE);
        let (producer, mut consumer) = rb.split();

        let is_playing = Arc::new(AtomicBool::new(false));
        let playing_flag = is_playing.clone();
        let channels = device_channels as usize;

        let stream = match sample_format {
            SampleFormat::F32 => device.build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    if !playing_flag.load(Ordering::Relaxed) {
                        // Output silence
                        data.fill(0.0);
                        return;
                    }
                    // Read mono samples from ring buffer and duplicate to all channels
                    let frames = data.len() / channels;
                    for frame in 0..frames {
                        let sample = consumer.try_pop().unwrap_or(0.0);
                        for ch in 0..channels {
                            data[frame * channels + ch] = sample;
                        }
                    }
                },
                |err| eprintln!("audio playback error: {err}"),
                None,
            ),
            _ => return Err(format!("unsupported output sample format: {sample_format:?}")),
        }
        .map_err(|e| format!("failed to build output stream: {e}"))?;

        stream
            .play()
            .map_err(|e| format!("failed to start playback stream: {e}"))?;

        Ok(Self {
            stream,
            producer,
            is_playing,
            device_rate,
        })
    }

    /// Queue audio samples for playback. Input should be f32 mono at `sample_rate`.
    /// Will be resampled to the device's native rate if needed.
    pub fn queue_audio(&mut self, samples: &[f32], sample_rate: u32) {
        let resampled = if sample_rate != self.device_rate {
            super::resampler::resample_mono(samples, sample_rate, self.device_rate)
        } else {
            samples.to_vec()
        };

        self.is_playing.store(true, Ordering::Relaxed);
        self.resume();
        for &sample in &resampled {
            let _ = self.producer.try_push(sample);
        }
    }

    /// Flush the playback buffer and stop playing.
    pub fn flush(&mut self) {
        self.is_playing.store(false, Ordering::Relaxed);
        let _ = self.stream.pause();
    }

    /// Resume the output stream (called automatically by queue_audio).
    pub fn resume(&self) {
        let _ = self.stream.play();
    }

    /// Check if audio is currently playing.
    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::Relaxed)
    }
}
