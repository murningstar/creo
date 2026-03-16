use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossbeam_channel::{bounded, Receiver, Sender};
use tauri::{AppHandle, Emitter};

use super::capture::AudioResampler;
use super::transcriber::Transcriber;
use super::vad::SileroVad;
use super::{
    AudioMode, AudioStatePayload, ErrorPayload, PipelineHandle, TranscriptionPayload,
    TranscriptionRequest, TranscriptionResult, VadStatePayload, WakeCommand, WakeCommandPayload,
};

const VAD_SILENCE_TIMEOUT_MS: u64 = 300;
const MIN_SPEECH_SAMPLES: usize = 4000; // ~250ms at 16kHz

pub fn start_pipeline(
    app_handle: AppHandle,
    handle: Arc<PipelineHandle>,
    vad_model_path: String,
    whisper_model_path: String,
) -> Result<()> {
    let shutdown = handle.shutdown.clone();
    let mode = handle.mode.clone();

    // Set initial mode
    handle.set_mode(AudioMode::Listening);
    let _ = app_handle.emit(
        "audio-state-changed",
        AudioStatePayload {
            mode: AudioMode::Listening,
        },
    );

    // Channels between processing ↔ transcription threads
    let (trans_tx, trans_rx): (Sender<TranscriptionRequest>, Receiver<TranscriptionRequest>) =
        bounded(4);
    let (result_tx, result_rx): (Sender<TranscriptionResult>, Receiver<TranscriptionResult>) =
        bounded(4);

    // Transcription thread
    let app2 = app_handle.clone();
    let shutdown2 = shutdown.clone();
    let transcription_handle = thread::spawn(move || {
        if let Err(e) =
            transcription_thread(app2, trans_rx, result_tx, shutdown2, &whisper_model_path)
        {
            log::error!("Transcription thread error: {}", e);
        }
    });

    // Processing thread — creates cpal capture internally (Stream is !Send)
    let app1 = app_handle.clone();
    let shutdown1 = shutdown.clone();
    let mode1 = mode.clone();
    let processing_handle = thread::spawn(move || {
        if let Err(e) = processing_thread(app1, trans_tx, result_rx, shutdown1, mode1, &vad_model_path)
        {
            log::error!("Processing thread error: {}", e);
        }
    });

    let mut threads = handle.threads.lock().unwrap();
    threads.push(processing_handle);
    threads.push(transcription_handle);

    log::info!("Pipeline started");
    Ok(())
}

fn processing_thread(
    app: AppHandle,
    trans_tx: Sender<TranscriptionRequest>,
    result_rx: Receiver<TranscriptionResult>,
    shutdown: Arc<AtomicBool>,
    mode: Arc<AtomicU8>,
    vad_model_path: &str,
) -> Result<()> {
    use super::capture::AudioCapture;

    // Create capture on THIS thread (cpal::Stream is !Send)
    let (_capture, audio_rx, native_rate) = AudioCapture::start()?;

    let mut resampler = AudioResampler::new(native_rate)?;
    let mut vad = SileroVad::new(vad_model_path)?;

    let mut vad_buffer: Vec<f32> = Vec::new();
    let mut speech_buffer: Vec<f32> = Vec::new();
    let mut is_speaking = false;
    let mut silence_start: Option<Instant> = None;
    let chunk_size = SileroVad::chunk_size();

    log::info!(
        "Processing thread started (native rate: {}Hz)",
        native_rate
    );

    loop {
        if shutdown.load(Ordering::Relaxed) {
            let _ = trans_tx.send(TranscriptionRequest::Shutdown);
            break;
        }

        // Check for transcription results (non-blocking)
        while let Ok(result) = result_rx.try_recv() {
            let current_mode = AudioMode::from_u8(mode.load(Ordering::Relaxed));
            handle_transcription_result(&app, &mode, result, current_mode);
        }

        // Receive audio (with timeout to check shutdown periodically)
        let chunk = match audio_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(chunk) => chunk,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
        };

        let current_mode = AudioMode::from_u8(mode.load(Ordering::Relaxed));
        if current_mode == AudioMode::Idle {
            continue;
        }

        // Resample to 16kHz
        let resampled = resampler.process(&chunk);
        vad_buffer.extend_from_slice(&resampled);

        // Process VAD in 512-sample chunks
        while vad_buffer.len() >= chunk_size {
            let vad_chunk: Vec<f32> = vad_buffer.drain(..chunk_size).collect();

            let speech = match vad.is_speech(&vad_chunk) {
                Ok(s) => s,
                Err(e) => {
                    log::error!("VAD error: {}", e);
                    continue;
                }
            };

            let _ = app.emit("vad-state", VadStatePayload { is_speech: speech });

            if speech {
                silence_start = None;
                if !is_speaking {
                    is_speaking = true;
                    log::debug!("Speech started");
                }
                speech_buffer.extend_from_slice(&vad_chunk);
            } else if is_speaking {
                // Accumulate during short silence gap
                speech_buffer.extend_from_slice(&vad_chunk);

                if silence_start.is_none() {
                    silence_start = Some(Instant::now());
                }

                if let Some(start) = silence_start {
                    if start.elapsed() > Duration::from_millis(VAD_SILENCE_TIMEOUT_MS) {
                        // Silence timeout — send accumulated speech for processing
                        is_speaking = false;
                        silence_start = None;

                        log::info!("Speech ended, {} samples buffered", speech_buffer.len());

                        if speech_buffer.len() >= MIN_SPEECH_SAMPLES {
                            let buffer = std::mem::take(&mut speech_buffer);
                            let current = AudioMode::from_u8(mode.load(Ordering::Relaxed));

                            match current {
                                AudioMode::Listening => {
                                    mode.store(AudioMode::Processing.as_u8(), Ordering::Relaxed);
                                    let _ = app.emit(
                                        "audio-state-changed",
                                        AudioStatePayload {
                                            mode: AudioMode::Processing,
                                        },
                                    );
                                    let _ =
                                        trans_tx.send(TranscriptionRequest::WakeWordCheck(buffer));
                                }
                                AudioMode::Dictation => {
                                    let _ =
                                        trans_tx.send(TranscriptionRequest::DictationChunk(buffer));
                                }
                                _ => {}
                            }
                        } else {
                            speech_buffer.clear();
                        }

                        vad.reset();
                    }
                }
            }
        }
    }

    log::info!("Processing thread exiting");
    // _capture (cpal::Stream) drops here, on this thread
    Ok(())
}

fn handle_transcription_result(
    app: &AppHandle,
    mode: &Arc<AtomicU8>,
    result: TranscriptionResult,
    current_mode: AudioMode,
) {
    match result {
        TranscriptionResult::WakeCommand(cmd) => {
            log::info!("Wake command: {:?}", cmd);
            let _ = app.emit("wake-command", WakeCommandPayload { command: cmd });

            match cmd {
                WakeCommand::Vpisyvai => {
                    mode.store(AudioMode::Dictation.as_u8(), Ordering::Relaxed);
                    let _ = app.emit(
                        "audio-state-changed",
                        AudioStatePayload {
                            mode: AudioMode::Dictation,
                        },
                    );
                }
                WakeCommand::Gotovo => {
                    mode.store(AudioMode::Listening.as_u8(), Ordering::Relaxed);
                    let _ = app.emit(
                        "audio-state-changed",
                        AudioStatePayload {
                            mode: AudioMode::Listening,
                        },
                    );
                }
                WakeCommand::Priem => {
                    mode.store(AudioMode::Listening.as_u8(), Ordering::Relaxed);
                    let _ = app.emit(
                        "audio-state-changed",
                        AudioStatePayload {
                            mode: AudioMode::Listening,
                        },
                    );
                }
            }
        }
        TranscriptionResult::DictationText { text, is_final } => {
            let _ = app.emit("transcription", TranscriptionPayload { text, is_final });
        }
        TranscriptionResult::NoMatch => {
            if current_mode == AudioMode::Processing {
                mode.store(AudioMode::Listening.as_u8(), Ordering::Relaxed);
                let _ = app.emit(
                    "audio-state-changed",
                    AudioStatePayload {
                        mode: AudioMode::Listening,
                    },
                );
            }
        }
    }
}

fn transcription_thread(
    app: AppHandle,
    rx: Receiver<TranscriptionRequest>,
    result_tx: Sender<TranscriptionResult>,
    shutdown: Arc<AtomicBool>,
    whisper_model_path: &str,
) -> Result<()> {
    let transcriber = Transcriber::new(whisper_model_path)?;
    log::info!("Transcription thread started, model loaded");

    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        let request = match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(req) => req,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
        };

        match request {
            TranscriptionRequest::WakeWordCheck(audio) => {
                match transcriber.transcribe(&audio, "ru") {
                    Ok(text) => {
                        log::info!("Wake word check: '{}'", text);
                        if let Some(cmd) = super::transcriber::match_wake_word(&text) {
                            let _ = result_tx.send(TranscriptionResult::WakeCommand(cmd));
                        } else {
                            let _ = result_tx.send(TranscriptionResult::NoMatch);
                        }
                    }
                    Err(e) => {
                        log::error!("Transcription error: {}", e);
                        let _ = app.emit(
                            "error",
                            ErrorPayload {
                                message: e.to_string(),
                            },
                        );
                        let _ = result_tx.send(TranscriptionResult::NoMatch);
                    }
                }
            }
            TranscriptionRequest::DictationChunk(audio) => {
                match transcriber.transcribe(&audio, "ru") {
                    Ok(text) => {
                        if let Some(WakeCommand::Gotovo) =
                            super::transcriber::match_wake_word(&text)
                        {
                            let _ = result_tx
                                .send(TranscriptionResult::WakeCommand(WakeCommand::Gotovo));
                        } else {
                            let trimmed = text.trim().to_string();
                            if !trimmed.is_empty() {
                                let _ = result_tx.send(TranscriptionResult::DictationText {
                                    text: trimmed,
                                    is_final: true,
                                });
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Dictation error: {}", e);
                        let _ = app.emit(
                            "error",
                            ErrorPayload {
                                message: e.to_string(),
                            },
                        );
                    }
                }
            }
            TranscriptionRequest::Shutdown => break,
        }
    }

    log::info!("Transcription thread exiting");
    Ok(())
}
