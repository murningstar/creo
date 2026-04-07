use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::Stream;
use crossbeam_channel::{bounded, Receiver, Sender};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::path::Path;
use std::time::{Duration, Instant};

const TARGET_SAMPLE_RATE: u32 = 16000;
const CHANNEL_CAPACITY: usize = 64;

/// Rubato SincFixedIn quality/performance tradeoff parameters.
const RESAMPLER_SINC_LEN: usize = 256;
const RESAMPLER_CUTOFF: f32 = 0.95;
const RESAMPLER_OVERSAMPLING: usize = 256;
/// Minimum chunk size for rubato — 480 samples = 30ms at 16kHz.
const MIN_RESAMPLER_CHUNK: usize = 480;

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
                if tx.try_send(mono).is_err() {
                    static DROP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                    let count = DROP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if count % 100 == 0 {
                        log::warn!("Audio capture: dropped {} chunks (channel full)", count + 1);
                    }
                }
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
        let chunk_size = (source_rate as usize / 100).max(MIN_RESAMPLER_CHUNK); // ~10ms chunks, clamped to rubato minimum

        let params = SincInterpolationParameters {
            sinc_len: RESAMPLER_SINC_LEN,
            f_cutoff: RESAMPLER_CUTOFF,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: RESAMPLER_OVERSAMPLING,
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

/// Capture speech via VAD: wait for speech onset, record until silence timeout.
/// Returns the speech audio buffer (16kHz mono f32).
///
/// Parameters:
/// - `vad_model_path`: path to the Silero VAD ONNX model
/// - `timeout_secs`: maximum seconds to wait for speech before giving up
/// - `silence_ms`: milliseconds of silence after speech to consider speech ended
/// - `label`: label for log messages (e.g., "Wake sample", "Subcommand sample")
pub fn capture_speech_vad(
    vad_model_path: &Path,
    timeout_secs: u64,
    silence_ms: u64,
    label: &str,
) -> Result<Vec<f32>, String> {
    use super::vad::SileroVad;

    let (capture, rx, sample_rate) = AudioCapture::start().map_err(|e| e.to_string())?;
    let mut resampler = AudioResampler::new(sample_rate).map_err(|e| e.to_string())?;
    let mut vad =
        SileroVad::new(&vad_model_path.to_string_lossy()).map_err(|e| e.to_string())?;

    let chunk_size = SileroVad::chunk_size();
    let mut vad_buffer: Vec<f32> = Vec::new();
    let mut speech_buffer: Vec<f32> = Vec::new();
    let mut is_speaking = false;
    let mut silence_start: Option<Instant> = None;
    let mut speech_detected = false;
    let timeout = Instant::now();
    let silence_duration = Duration::from_millis(silence_ms);

    while timeout.elapsed() < Duration::from_secs(timeout_secs) {
        if let Ok(chunk) = rx.recv_timeout(Duration::from_millis(50)) {
            let resampled = resampler.process(&chunk);
            vad_buffer.extend_from_slice(&resampled);

            while vad_buffer.len() >= chunk_size {
                let vad_chunk: Vec<f32> = vad_buffer.drain(..chunk_size).collect();
                let is_speech = vad.is_speech(&vad_chunk).unwrap_or(false);

                if is_speech {
                    silence_start = None;
                    if !is_speaking {
                        is_speaking = true;
                        speech_detected = true;
                        log::info!("{}: speech started", label);
                    }
                    speech_buffer.extend_from_slice(&vad_chunk);
                } else if is_speaking {
                    speech_buffer.extend_from_slice(&vad_chunk);
                    if silence_start.is_none() {
                        silence_start = Some(Instant::now());
                    }
                    if let Some(start) = silence_start {
                        if start.elapsed() > silence_duration {
                            log::info!(
                                "{}: speech ended, {} samples",
                                label,
                                speech_buffer.len()
                            );
                            break;
                        }
                    }
                }
            }

            if speech_detected && !is_speaking {
                break;
            }
            if is_speaking
                && silence_start
                    .map(|s| s.elapsed() > silence_duration)
                    .unwrap_or(false)
            {
                break;
            }
        }
    }

    drop(capture);

    if speech_buffer.is_empty() {
        return Err("No speech detected. Please try again.".to_string());
    }

    Ok(speech_buffer)
}
