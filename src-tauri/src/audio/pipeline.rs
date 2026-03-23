use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossbeam_channel::{bounded, Receiver, Sender};
use tauri::{AppHandle, Emitter};

use super::capture::AudioResampler;
use super::transcriber::Transcriber;
use super::vad::SileroVad;
use super::wakeword::WakeWordDetector;
use super::{
    AudioMode, ErrorPayload, PipelineHandle, TranscriptionPayload, TranscriptionRequest,
    TranscriptionResult, VadStatePayload, WakeCommand, WakeCommandPayload,
};

const VAD_SILENCE_TIMEOUT_MS: u64 = 300;
const MIN_SPEECH_SAMPLES: usize = 4000; // ~250ms at 16kHz
const TRANSCRIPTION_CHANNEL_CAPACITY: usize = 4;

pub fn start_pipeline(
    app_handle: AppHandle,
    handle: Arc<PipelineHandle>,
    vad_model_path: String,
    mel_model_path: String,
    emb_model_path: String,
    wakewords_dir: String,
    dictation_model_path: String,
) -> Result<()> {
    handle.transition_mode(&app_handle, AudioMode::Listening);

    // Channels between processing ↔ transcription threads
    let (trans_tx, trans_rx): (Sender<TranscriptionRequest>, Receiver<TranscriptionRequest>) =
        bounded(TRANSCRIPTION_CHANNEL_CAPACITY);
    let (result_tx, result_rx): (Sender<TranscriptionResult>, Receiver<TranscriptionResult>) =
        bounded(TRANSCRIPTION_CHANNEL_CAPACITY);

    // Transcription thread (wake word via embedding+DTW, dictation via whisper)
    let app2 = app_handle.clone();
    let handle2 = handle.clone();
    let transcription_handle = thread::spawn(move || {
        if let Err(e) = transcription_thread(
            app2,
            handle2,
            trans_rx,
            result_tx,
            &mel_model_path,
            &emb_model_path,
            &wakewords_dir,
            &dictation_model_path,
        ) {
            log::error!("Transcription thread error: {}", e);
        }
    });

    // Processing thread — creates cpal capture internally (Stream is !Send)
    let app1 = app_handle.clone();
    let handle1 = handle.clone();
    let processing_handle = thread::spawn(move || {
        if let Err(e) = processing_thread(app1, handle1, trans_tx, result_rx, &vad_model_path) {
            log::error!("Processing thread error: {}", e);
        }
    });

    handle
        .push_thread(processing_handle)
        .map_err(|e| anyhow::anyhow!(e))?;
    handle
        .push_thread(transcription_handle)
        .map_err(|e| anyhow::anyhow!(e))?;

    log::info!("Pipeline started");
    Ok(())
}

fn processing_thread(
    app: AppHandle,
    handle: Arc<PipelineHandle>,
    trans_tx: Sender<TranscriptionRequest>,
    result_rx: Receiver<TranscriptionResult>,
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
        if handle.is_shutdown() {
            let _ = trans_tx.send(TranscriptionRequest::Shutdown);
            break;
        }

        // Check for transcription results (non-blocking)
        while let Ok(result) = result_rx.try_recv() {
            handle_transcription_result(&app, &handle, result);
        }

        // Receive audio (with timeout to check shutdown periodically)
        let chunk = match audio_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(chunk) => chunk,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
        };

        if handle.current_mode() == AudioMode::Idle {
            continue;
        }

        // Resample to 16kHz
        let resampled = resampler.process(&chunk);

        // TEMPORARY: log audio RMS every ~1 sec to confirm mic captures speech
        // TODO: remove after confirming pipeline works end-to-end
        static AUDIO_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let ac = AUDIO_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if ac % 30 == 0 && !resampled.is_empty() {
            let rms = (resampled.iter().map(|s| s * s).sum::<f32>() / resampled.len() as f32).sqrt();
            log::info!("Audio RMS: {:.6} (resampled {} samples)", rms, resampled.len());
        }

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
                    log::info!("Speech started");
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

                            match handle.current_mode() {
                                AudioMode::Listening => {
                                    handle.transition_mode(&app, AudioMode::Processing);
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
    handle: &PipelineHandle,
    result: TranscriptionResult,
) {
    match result {
        TranscriptionResult::WakeCommand(cmd) => {
            log::info!("Wake command: {:?}", cmd);
            let _ = app.emit("wake-command", WakeCommandPayload { command: cmd });

            match cmd {
                WakeCommand::StartDictation => handle.transition_mode(app, AudioMode::Dictation),
                WakeCommand::StopDictation | WakeCommand::CommandMode => {
                    handle.transition_mode(app, AudioMode::Listening)
                }
            }
        }
        TranscriptionResult::DictationText { text, is_final } => {
            let _ = app.emit("transcription", TranscriptionPayload { text, is_final });
        }
        TranscriptionResult::NoMatch => {
            if handle.current_mode() == AudioMode::Processing {
                handle.transition_mode(app, AudioMode::Listening);
            }
        }
    }
}

fn transcription_thread(
    app: AppHandle,
    handle: Arc<PipelineHandle>,
    rx: Receiver<TranscriptionRequest>,
    result_tx: Sender<TranscriptionResult>,
    mel_model_path: &str,
    emb_model_path: &str,
    wakewords_dir: &str,
    dictation_model_path: &str,
) -> Result<()> {
    let mut wake_detector = WakeWordDetector::new(mel_model_path, emb_model_path, wakewords_dir)?;
    log::info!("Wake word detector loaded (embedding+DTW)");
    let dictation_transcriber = Transcriber::new(dictation_model_path)?;
    log::info!("Dictation model loaded: {}", dictation_model_path);

    loop {
        if handle.is_shutdown() {
            break;
        }

        let request = match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(req) => req,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
        };

        match request {
            TranscriptionRequest::WakeWordCheck(audio) => {
                if !wake_detector.has_references() {
                    log::debug!("No wake word references loaded, skipping detection");
                    let _ = result_tx.send(TranscriptionResult::NoMatch);
                } else {
                    match wake_detector.detect(&audio) {
                        Some(cmd) => {
                            let _ = result_tx.send(TranscriptionResult::WakeCommand(cmd));
                        }
                        None => {
                            let _ = result_tx.send(TranscriptionResult::NoMatch);
                        }
                    }
                }
            }
            TranscriptionRequest::DictationChunk(audio) => {
                // First check if this is a "gotovo" command via embedding+DTW
                if wake_detector.has_references() {
                    if let Some(WakeCommand::StopDictation) = wake_detector.detect(&audio) {
                        let _ = result_tx
                            .send(TranscriptionResult::WakeCommand(WakeCommand::StopDictation));
                        continue;
                    }
                }

                // Not a stop command — transcribe as dictation text
                match dictation_transcriber.transcribe(&audio, "ru") {
                    Ok(text) => {
                        let trimmed = text.trim().to_string();
                        if !trimmed.is_empty() {
                            let _ = result_tx.send(TranscriptionResult::DictationText {
                                text: trimmed,
                                is_final: true,
                            });
                        }
                    }
                    Err(e) => {
                        log::error!("Dictation error: {}", e);
                        let _ = app.emit(
                            "audio-error",
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
