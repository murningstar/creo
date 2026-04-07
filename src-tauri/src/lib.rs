mod audio;
mod commands;
mod input;
mod system;

use std::sync::Arc;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, RunEvent, WindowEvent};
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
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_denylist(&["overlay"])
                .build(),
        )
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
            let log_level = if cfg!(debug_assertions) {
                log::LevelFilter::Info
            } else {
                log::LevelFilter::Warn
            };
            app.handle().plugin(
                tauri_plugin_log::Builder::default()
                    .level(log_level)
                    .build(),
            )?;

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

            // Position overlay window in bottom-right corner (above taskbar)
            if let Some(overlay) = app.get_webview_window("overlay") {
                let is_wayland = system::detect::detect_display_server() == system::detect::DisplayServer::Wayland;

                if is_wayland {
                    // Wayland: transparent alwaysOnTop windows cause tao/GTK event loop panics.
                    // Keep overlay hidden (created from config with visible: false).
                    log::warn!(
                        "Wayland detected: overlay disabled (transparent alwaysOnTop unreliable). \
                         See .claude/docs/platform.md for details."
                    );
                } else {
                    if let Ok(monitor) = overlay.current_monitor() {
                        if let Some(monitor) = monitor {
                            let screen = monitor.size();
                            let mon_pos = monitor.position();
                            let scale = monitor.scale_factor();

                            // Window is 96x96 with 48px circle centered inside.
                            // The 24px transparent padding around the circle IS the margin.
                            // Position window flush to screen edge — no Rust margin needed.
                            // Invisible borders extend beyond screen edge (they're invisible).
                            let window_phys = (96.0 * scale) as i32;
                            let taskbar_phys = (48.0 * scale) as i32;

                            let x = mon_pos.x + screen.width as i32 - window_phys;
                            let y = mon_pos.y + screen.height as i32
                                - window_phys
                                - taskbar_phys;

                            if let Err(e) = overlay.set_position(tauri::Position::Physical(
                                tauri::PhysicalPosition::new(x, y),
                            )) {
                                log::warn!("Failed to position overlay window: {e}");
                            }
                        }
                    }
                    // Enable click-through by default
                    if let Err(e) = overlay.set_ignore_cursor_events(true) {
                        log::warn!("Failed to set overlay click-through: {e}");
                        let _ = app.emit("overlay-capability-degraded", serde_json::json!({
                            "capability": "click_through",
                            "error": e.to_string(),
                        }));
                    }

                    // Dev: keep in Alt+Tab for F12 DevTools access
                    // Prod: hide from taskbar/Alt+Tab
                    if !cfg!(debug_assertions) {
                        if let Err(e) = overlay.set_skip_taskbar(true) {
                            log::warn!("Failed to hide overlay from taskbar: {e}");
                        }
                    }

                    // Show the overlay window (starts hidden in config)
                    if let Err(e) = overlay.show() {
                        log::warn!("Failed to show overlay window: {e}");
                    }
                }
            }

            // --- System tray ---
            let show_item = MenuItem::with_id(app, "show", "Show Dashboard", true, None::<&str>)?;
            let separator = PredefinedMenuItem::separator(app)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&show_item, &separator, &quit_item])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().expect("default window icon must be set in tauri.conf.json5").clone())
                .tooltip("Creo")
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            match event {
                // Hide main window to tray instead of quitting
                RunEvent::WindowEvent {
                    label,
                    event: WindowEvent::CloseRequested { api, .. },
                    ..
                } if label == "main" => {
                    api.prevent_close();
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.hide();
                    }
                }

                RunEvent::Exit => {
                    // Stop model polling thread
                    let polling = app.state::<std::sync::Arc<std::sync::atomic::AtomicBool>>();
                    polling.store(true, std::sync::atomic::Ordering::Relaxed);

                    // Stop audio pipeline
                    let handle = app.state::<Arc<audio::PipelineHandle>>();
                    handle.request_shutdown();
                    let _ = handle.join_threads();
                    log::info!("Pipeline shutdown complete");
                }

                _ => {}
            }
        });
}
