use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::Stream;
use crossbeam_channel::{bounded, Receiver, Sender};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

const TARGET_SAMPLE_RATE: u32 = 16000;
const CHANNEL_CAPACITY: usize = 64;

pub struct AudioCapture {
    _stream: Stream,
}

impl AudioCapture {
    /// Start capturing audio from the default input device.
    /// Returns the capture handle, a receiver for mono f32 audio, and the native sample rate.
    pub fn start() -> Result<(Self, Receiver<Vec<f32>>, u32)> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .context("No input device available")?;

        let config = device.default_input_config()?;
        let sample_rate = config.sample_rate().0;
        let channels = config.channels() as usize;

        log::info!(
            "Audio capture: device={}, sample_rate={}, channels={}",
            device.name().unwrap_or_default(),
            sample_rate,
            channels
        );

        let (tx, rx): (Sender<Vec<f32>>, Receiver<Vec<f32>>) = bounded(CHANNEL_CAPACITY);

        let stream = device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                // Mix to mono if multi-channel
                let mono: Vec<f32> = if channels == 1 {
                    data.to_vec()
                } else {
                    data.chunks(channels)
                        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                        .collect()
                };

                // Non-blocking send — drop data if channel is full
                let _ = tx.try_send(mono);
            },
            |err| {
                log::error!("Audio capture error: {}", err);
            },
            None,
        )?;

        stream.play()?;

        Ok((Self { _stream: stream }, rx, sample_rate))
    }
}

/// Resamples audio from native sample rate to 16kHz mono.
pub struct AudioResampler {
    resampler: Option<SincFixedIn<f32>>,
    input_buffer: Vec<f32>,
    chunk_size: usize,
}

impl AudioResampler {
    pub fn new(source_rate: u32) -> Result<Self> {
        if source_rate == TARGET_SAMPLE_RATE {
            return Ok(Self {
                resampler: None,
                input_buffer: Vec::new(),
                chunk_size: 0,
            });
        }

        let ratio = TARGET_SAMPLE_RATE as f64 / source_rate as f64;
        let chunk_size = (source_rate as usize / 100).max(480); // ~10ms chunks

        let params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 256,
            window: WindowFunction::BlackmanHarris2,
        };

        let resampler =
            SincFixedIn::new(ratio, 2.0, params, chunk_size, 1).context("Failed to create resampler")?;

        Ok(Self {
            resampler: Some(resampler),
            input_buffer: Vec::with_capacity(chunk_size * 2),
            chunk_size,
        })
    }

    /// Feed raw mono audio and get resampled 16kHz output.
    pub fn process(&mut self, input: &[f32]) -> Vec<f32> {
        let resampler = match &mut self.resampler {
            Some(r) => r,
            None => return input.to_vec(), // Already 16kHz
        };

        self.input_buffer.extend_from_slice(input);
        let mut output = Vec::new();

        while self.input_buffer.len() >= self.chunk_size {
            let chunk: Vec<f32> = self.input_buffer.drain(..self.chunk_size).collect();
            match resampler.process(&[&chunk], None) {
                Ok(resampled) => {
                    if let Some(channel) = resampled.first() {
                        output.extend_from_slice(channel);
                    }
                }
                Err(e) => {
                    log::error!("Resampling error: {}", e);
                }
            }
        }

        output
    }
}
