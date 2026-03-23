use anyhow::Result;
use enigo::{Enigo, Keyboard, Settings};

use super::injector::TextInjector;

pub struct TypeInjector;

impl TextInjector for TypeInjector {
    fn inject(&self, text: &str) -> Result<()> {
        if text.is_empty() {
            return Ok(());
        }

        let mut enigo = Enigo::new(&Settings::default()).map_err(|e| anyhow::anyhow!("{e}"))?;

        // enigo.text() batches all characters into a single SendInput call on Windows.
        // On macOS, it splits into 20-char CGEvent chunks.
        // On Linux X11, it uses XTest key simulation.
        enigo
            .text(text)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        Ok(())
    }
}
