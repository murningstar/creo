use anyhow::{Context, Result};
use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use std::thread;
use std::time::Duration;

use super::injector::TextInjector;

pub struct PasteInjector;

impl TextInjector for PasteInjector {
    fn inject(&self, text: &str) -> Result<()> {
        if text.is_empty() {
            return Ok(());
        }

        // 1. Write text to clipboard
        let mut clipboard = Clipboard::new().context("Failed to open clipboard")?;
        clipboard
            .set_text(text)
            .context("Failed to write text to clipboard")?;

        // 2. Platform-specific paste delay
        thread::sleep(paste_delay());

        // 3. Simulate paste shortcut
        simulate_paste().context("Failed to simulate paste keystroke")?;

        // 4. Settle delay (let the app process the paste)
        thread::sleep(settle_delay());

        Ok(())
    }
}

/// Simulate the paste keyboard shortcut per platform.
fn simulate_paste() -> Result<()> {
    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| anyhow::anyhow!("{e}"))?;

    #[cfg(target_os = "macos")]
    {
        // Cmd+V on macOS
        enigo
            .key(Key::Meta, Direction::Press)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        enigo
            .key(Key::Unicode('v'), Direction::Click)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        enigo
            .key(Key::Meta, Direction::Release)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
    }

    #[cfg(target_os = "windows")]
    {
        // Release any held modifier keys first (OpenWhispr lesson #438)
        release_held_modifiers(&mut enigo)?;

        // Ctrl+V on Windows
        enigo
            .key(Key::Control, Direction::Press)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        enigo
            .key(Key::Unicode('v'), Direction::Click)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        enigo
            .key(Key::Control, Direction::Release)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
    }

    #[cfg(target_os = "linux")]
    {
        let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok()
            || std::env::var("XDG_SESSION_TYPE")
                .map(|v| v == "wayland")
                .unwrap_or(false);

        if is_wayland {
            // Wayland: Ctrl+Shift+V (works in terminals and most GUI apps).
            // Known limitation: may fail with non-English keyboard layout —
            // enigo simulates keycodes mapped to active layout, not physical 'v'.
            // See .claude/docs/platform.md for details.
            static WAYLAND_WARN: std::sync::Once = std::sync::Once::new();
            WAYLAND_WARN.call_once(|| {
                log::warn!(
                    "Wayland paste: Ctrl+Shift+V may fail with non-English keyboard layout. \
                     Consider switching to Type input method in Settings."
                );
            });

            enigo
                .key(Key::Control, Direction::Press)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            enigo
                .key(Key::Shift, Direction::Press)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            enigo
                .key(Key::Unicode('v'), Direction::Click)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            enigo
                .key(Key::Shift, Direction::Release)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            enigo
                .key(Key::Control, Direction::Release)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
        } else {
            // X11: standard Ctrl+V (XTest handles this correctly)
            enigo
                .key(Key::Control, Direction::Press)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            enigo
                .key(Key::Unicode('v'), Direction::Click)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            enigo
                .key(Key::Control, Direction::Release)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
        }
    }

    Ok(())
}

/// Release any modifier keys the user might be holding down.
/// Without this, SendInput on Windows sends 'v' instead of Ctrl+V
/// if the user is holding a modifier key from a hotkey trigger.
#[cfg(target_os = "windows")]
fn release_held_modifiers(enigo: &mut Enigo) -> Result<()> {
    use enigo::Key;

    let modifiers = [Key::Control, Key::Shift, Key::Alt];
    for key in &modifiers {
        // Release just in case — if not held, this is a no-op
        let _ = enigo.key(*key, Direction::Release);
    }
    // Small delay to let the OS process the releases
    thread::sleep(Duration::from_millis(5));
    Ok(())
}

fn paste_delay() -> Duration {
    if cfg!(target_os = "windows") {
        Duration::from_millis(10)
    } else if cfg!(target_os = "macos") {
        Duration::from_millis(120)
    } else {
        Duration::from_millis(50)
    }
}

fn settle_delay() -> Duration {
    if cfg!(target_os = "windows") {
        Duration::from_millis(80)
    } else if cfg!(target_os = "macos") {
        Duration::from_millis(200)
    } else {
        Duration::from_millis(200)
    }
}
