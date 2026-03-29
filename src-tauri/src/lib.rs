mod audio;
mod commands;
mod input;
mod system;

use std::sync::Arc;

use tauri::{Emitter, Manager, RunEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Default hotkey: Ctrl+` (backtick)
    // Chosen for: no conflicts on any platform, no Shift (avoids RU layout switch),
    // available on all keyboards. See .claude/docs/hotkeys/cross-platform-summary.md
    let default_hotkey = Shortcut::new(Some(Modifiers::CONTROL), Code::Backquote);

    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_shortcut(default_hotkey)
                .unwrap_or_else(|e| {
                    log::error!("Failed to register default hotkey: {}", e);
                    tauri_plugin_global_shortcut::Builder::new()
                })
                .with_handler(move |app, shortcut, event| {
                    if shortcut == &default_hotkey {
                        match event.state() {
                            ShortcutState::Pressed => {
                                log::info!("Hotkey pressed");
                                let _ = app.emit("hotkey-pressed", ());
                            }
                            ShortcutState::Released => {
                                log::info!("Hotkey released");
                                let _ = app.emit("hotkey-released", ());
                            }
                        }
                    }
                })
                .build(),
        )
        .manage(Arc::new(audio::PipelineHandle::new()))
        .invoke_handler(tauri::generate_handler![
            commands::start_listening,
            commands::start_dictation,
            commands::transition_to_dictation,
            commands::transition_to_standby,
            commands::get_current_mode,
            commands::stop_listening,
            commands::test_capture,
            commands::check_models,
            commands::inject_text,
            commands::detect_system,
            commands::record_wake_sample,
            commands::get_wake_commands,
            commands::delete_wake_command,
            commands::get_subcommands,
            commands::create_subcommand,
            commands::delete_subcommand,
            commands::record_subcommand_sample,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // Background model file polling (5s interval)
            let handle = app.handle().clone();
            let polling_shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let polling_flag = polling_shutdown.clone();
            std::thread::spawn(move || {
                let mut prev_ready = false;
                while !polling_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    let status = commands::check_models();
                    if status.all_present != prev_ready {
                        prev_ready = status.all_present;
                        let _ = handle.emit("models-status-changed", status);
                    }
                    std::thread::sleep(std::time::Duration::from_secs(5));
                }
            });
            // Store polling shutdown flag for exit handler
            app.manage(polling_shutdown);

            // Position overlay window in bottom-right corner
            if let Some(overlay) = app.get_webview_window("overlay") {
                if let Ok(monitor) = overlay.current_monitor() {
                    if let Some(monitor) = monitor {
                        let screen = monitor.size();
                        let scale = monitor.scale_factor();
                        let margin = (12.0 * scale) as i32;
                        let size = (60.0 * scale) as i32;
                        let x = (screen.width as i32 / scale as i32) - size - margin;
                        let y = (screen.height as i32 / scale as i32) - size - margin;
                        let _ = overlay.set_position(tauri::Position::Logical(
                            tauri::LogicalPosition::new(x as f64, y as f64),
                        ));
                    }
                }
                // Enable click-through by default
                let _ = overlay.set_ignore_cursor_events(true);
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                // Stop model polling thread
                let polling = app.state::<std::sync::Arc<std::sync::atomic::AtomicBool>>();
                polling.store(true, std::sync::atomic::Ordering::Relaxed);

                // Stop audio pipeline
                let handle = app.state::<Arc<audio::PipelineHandle>>();
                handle.request_shutdown();
                let _ = handle.join_threads();
                log::info!("Pipeline shutdown complete");
            }
        });
}
