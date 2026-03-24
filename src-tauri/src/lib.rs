mod audio;
mod commands;
mod input;
mod system;

use std::sync::Arc;

use tauri::Emitter;
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
            commands::stop_listening,
            commands::test_capture,
            commands::check_models,
            commands::inject_text,
            commands::detect_system,
            commands::record_wake_sample,
            commands::get_wake_commands,
            commands::delete_wake_command,
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
            std::thread::spawn(move || {
                let mut prev_ready = false;
                loop {
                    let status = commands::check_models();
                    if status.all_present != prev_ready {
                        prev_ready = status.all_present;
                        let _ = handle.emit("models-status-changed", status);
                    }
                    std::thread::sleep(std::time::Duration::from_secs(5));
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
