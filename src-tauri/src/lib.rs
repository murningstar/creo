mod audio;
mod commands;
mod input;
mod system;

use std::sync::Arc;

use tauri::Emitter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
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
