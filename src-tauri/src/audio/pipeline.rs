use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossbeam_channel::{bounded, Receiver, Sender};
use tauri::{AppHandle, Emitter};

use super::capture::AudioResampler;
use super::stt::DictationEngine;
use super::subcommand::SubcommandCascade;
use super::vad::SileroVad;
use super::wakeword::WakeWordDetector;
use super::{
    AudioMode, ErrorPayload, PipelineHandle, SubcommandMatchPayload, TranscriptionPayload,
    TranscriptionRequest, TranscriptionResult, VadStatePayload, WakeAction, WakeActionPayload,
};

const SILENCE_TIMEOUT_STANDBY_MS: u64 = 300;
const SILENCE_TIMEOUT_DICTATION_MS: u64 = 800;
const MIN_SPEECH_SAMPLES: usize = 4000; // ~250ms at 16kHz
const TRANSCRIPTION_CHANNEL_CAPACITY: usize = 4;

/// Audio overlap: keep last 500ms of audio to prepend to next segment.
/// Prevents mid-word splits at VAD boundaries.
const OVERLAP_SAMPLES: usize = 8000; // 500ms at 16kHz

/// AwaitingSubcommand timeout: return to Standby after this many seconds without a match.
const SUBCOMMAND_TIMEOUT_SECS: u64 = 10;

/// Wake word debounce: minimum time between consecutive accepted detections (ms).
/// Prevents rapid-fire false positives during continuous speech.
/// Applied only to WakeWordCheck (Standby mode), NOT to DictationChunk (stop/cancel must work immediately).
const WAKE_DEBOUNCE_MS: u64 = 2000;

pub fn start_pipeline(
    app_handle: AppHandle,
    handle: Arc<PipelineHandle>,
    initial_mode: AudioMode,
    vad_model_path: String,
    mel_model_path: String,
    emb_model_path: String,
    wakewords_dir: String,
    subcommands_dir: String,
    dictation_engine_factory: Box<dyn FnOnce() -> Result<Box<dyn DictationEngine>> + Send>,
) -> Result<()> {
    handle.transition_mode(&app_handle, initial_mode);

    // Channels between processing ↔ transcription threads
    let (trans_tx, trans_rx): (Sender<TranscriptionRequest>, Receiver<TranscriptionRequest>) =
        bounded(TRANSCRIPTION_CHANNEL_CAPACITY);

    // Store sender in PipelineHandle so commands can send reload signals
    handle
        .set_trans_tx(trans_tx.clone())
        .map_err(|e| anyhow::anyhow!(e))?;
    let (result_tx, result_rx): (Sender<TranscriptionResult>, Receiver<TranscriptionResult>) =
        bounded(TRANSCRIPTION_CHANNEL_CAPACITY);

    // Transcription thread (wake word via embedding+DTW, subcommand cascade, dictation via STT engine)
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
            &subcommands_dir,
            dictation_engine_factory,
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

    // Audio overlap: rolling buffer of last 500ms for cross-segment continuity
    let mut pre_buffer: Vec<f32> = Vec::new();

    // AwaitingSubcommand timeout tracking
    let mut awaiting_since: Option<Instant> = None;

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

        // AwaitingSubcommand timeout
        match handle.current_mode() {
            AudioMode::AwaitingSubcommand => {
                if awaiting_since.is_none() {
                    awaiting_since = Some(Instant::now());
                }
                if let Some(since) = awaiting_since {
                    if since.elapsed() > Duration::from_secs(SUBCOMMAND_TIMEOUT_SECS) {
                        log::info!(
                            "AwaitingSubcommand timeout after {}s → Standby",
                            SUBCOMMAND_TIMEOUT_SECS
                        );
                        handle.transition_mode(&app, AudioMode::Standby);
                        let _ = app.emit("subcommand-timeout", ());
                        awaiting_since = None;
                    }
                }
            }
            _ => {
                awaiting_since = None;
            }
        }

        // Receive audio (with timeout to check shutdown periodically)
        let chunk = match audio_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(chunk) => chunk,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
        };

        if handle.current_mode() == AudioMode::Off {
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

            // Emit RMS amplitude for overlay waveform visualization
            if speech {
                let rms = (vad_chunk.iter().map(|&s| (s * s) as f64).sum::<f64>()
                    / vad_chunk.len() as f64)
                    .sqrt() as f32;
                // Normalize: typical speech RMS ~0.02-0.15, scale to 0.0-1.0
                let normalized = (rms * 8.0).min(1.0);
                let _ = app.emit("vad-amplitude", normalized);
            }

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
                    // Mode-dependent silence threshold
                    let timeout_ms = match handle.current_mode() {
                        AudioMode::Dictation => SILENCE_TIMEOUT_DICTATION_MS,
                        _ => SILENCE_TIMEOUT_STANDBY_MS,
                    };

                    if start.elapsed() > Duration::from_millis(timeout_ms) {
                        // Silence timeout — send accumulated speech for processing
                        is_speaking = false;
                        silence_start = None;

                        log::info!("Speech ended, {} samples buffered", speech_buffer.len());

                        if speech_buffer.len() >= MIN_SPEECH_SAMPLES {
                            let buffer = std::mem::take(&mut speech_buffer);

                            match handle.current_mode() {
                                AudioMode::Standby => {
                                    handle.save_pre_processing_mode();
                                    handle.transition_mode(&app, AudioMode::Processing);
                                    let _ =
                                        trans_tx.send(TranscriptionRequest::WakeWordCheck(buffer));
                                }
                                AudioMode::AwaitingSubcommand => {
                                    handle.save_pre_processing_mode();
                                    handle.transition_mode(&app, AudioMode::Processing);
                                    let _ =
                                        trans_tx.send(TranscriptionRequest::SubcommandCheck(buffer));
                                }
                                AudioMode::Dictation => {
                                    // Prepend overlap from previous segment
                                    let mut with_overlap = pre_buffer.clone();
                                    with_overlap.extend_from_slice(&buffer);

                                    // Save tail of current buffer as overlap for next segment
                                    let tail_start = buffer.len().saturating_sub(OVERLAP_SAMPLES);
                                    pre_buffer = buffer[tail_start..].to_vec();

                                    let _ = trans_tx
                                        .send(TranscriptionRequest::DictationChunk(with_overlap));
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
        TranscriptionResult::WakeAction(cmd) => {
            log::info!("Wake command: {:?}", cmd);
            let _ = app.emit("wake-command", WakeActionPayload { command: cmd });

            match cmd {
                WakeAction::StartDictation => handle.transition_mode(app, AudioMode::Dictation),
                WakeAction::StopDictation | WakeAction::CancelDictation => {
                    handle.transition_mode(app, AudioMode::Standby)
                }
                WakeAction::AwaitSubcommand => {
                    handle.transition_mode(app, AudioMode::AwaitingSubcommand)
                }
            }
        }
        TranscriptionResult::SubcommandMatch(m) => {
            log::info!(
                "Subcommand: '{}' action='{}' tier={}",
                m.command_name,
                m.action,
                m.tier
            );
            let _ = app.emit(
                "subcommand-match",
                SubcommandMatchPayload {
                    command: m.command_name,
                    action: m.action,
                    confidence: m.confidence,
                    tier: m.tier,
                    params: m.params,
                },
            );
            handle.transition_mode(app, AudioMode::Standby);
        }
        TranscriptionResult::DictationText { text, is_final } => {
            let _ = app.emit("transcription", TranscriptionPayload { text, is_final });
        }
        TranscriptionResult::NoMatch => {
            if handle.current_mode() == AudioMode::Processing {
                handle.transition_mode(app, handle.pre_processing_mode());
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
    subcommands_dir: &str,
    dictation_engine_factory: Box<dyn FnOnce() -> Result<Box<dyn DictationEngine>> + Send>,
) -> Result<()> {
    let mut wake_detector = WakeWordDetector::new(mel_model_path, emb_model_path, wakewords_dir)?;
    log::info!("Wake word detector loaded (embedding+DTW)");

    let mut subcommand_cascade =
        SubcommandCascade::new(subcommands_dir, mel_model_path, emb_model_path)?;
    log::info!("Subcommand cascade loaded");

    let mut dictation_engine = dictation_engine_factory()?;
    log::info!("Dictation engine loaded: {}", dictation_engine.name());

    // Debounce: suppress rapid-fire wake detections in Standby mode
    let mut last_wake_detection: Option<Instant> = None;

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
                        Some(detection) => {
                            // Debounce: suppress if too soon after last detection
                            if let Some(last) = last_wake_detection {
                                if last.elapsed() < Duration::from_millis(WAKE_DEBOUNCE_MS) {
                                    log::info!(
                                        "Wake detection debounced ({}ms since last)",
                                        last.elapsed().as_millis()
                                    );
                                    let _ = result_tx.send(TranscriptionResult::NoMatch);
                                    continue;
                                }
                            }
                            last_wake_detection = Some(Instant::now());
                            let _ = result_tx
                                .send(TranscriptionResult::WakeAction(detection.action));
                        }
                        None => {
                            let _ = result_tx.send(TranscriptionResult::NoMatch);
                        }
                    }
                }
            }
            TranscriptionRequest::SubcommandCheck(audio) => {
                // Priority 1: wake words still checked (user might say "Крео, вписывай")
                if wake_detector.has_references() {
                    if let Some(detection) = wake_detector.detect(&audio) {
                        let _ =
                            result_tx.send(TranscriptionResult::WakeAction(detection.action));
                        continue;
                    }
                }

                // Priority 2: subcommand cascade
                if subcommand_cascade.has_commands() {
                    if let Some(m) = subcommand_cascade.process(&audio) {
                        let _ = result_tx.send(TranscriptionResult::SubcommandMatch(m));
                        continue;
                    }
                }

                // No match in either
                let _ = result_tx.send(TranscriptionResult::NoMatch);
            }
            TranscriptionRequest::DictationChunk(audio) => {
                // First check if this is a stop/cancel command via embedding+DTW
                if wake_detector.has_references() {
                    if let Some(detection) = wake_detector.detect(&audio) {
                        if matches!(
                            detection.action,
                            WakeAction::StopDictation | WakeAction::CancelDictation
                        ) {
                            let _ = result_tx
                                .send(TranscriptionResult::WakeAction(detection.action));
                            dictation_engine.reset_context();
                            continue;
                        }
                    }
                }

                // Not a stop command — transcribe as dictation text
                match dictation_engine.transcribe(&audio) {
                    Ok(result) => {
                        if !result.text.is_empty() {
                            let _ = result_tx.send(TranscriptionResult::DictationText {
                                text: result.text,
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
            TranscriptionRequest::ReloadReferences => {
                match wake_detector.reload_references() {
                    Ok(()) => log::info!("Wake word references reloaded"),
                    Err(e) => log::error!("Failed to reload wake references: {}", e),
                }
                match subcommand_cascade.reload() {
                    Ok(()) => log::info!("Subcommand references reloaded"),
                    Err(e) => log::error!("Failed to reload subcommand references: {}", e),
                }
            }
            TranscriptionRequest::Shutdown => break,
        }
    }

    log::info!("Transcription thread exiting");
    Ok(())
}
