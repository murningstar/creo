use anyhow::Result;

use super::paste::PasteInjector;
use super::typer::TypeInjector;
use super::TextInputMethod;

pub trait TextInjector: Send {
    fn inject(&self, text: &str) -> Result<()>;
}

pub fn inject_text(text: &str, method: TextInputMethod) -> Result<()> {
    match method {
        TextInputMethod::Paste => {
            let injector = PasteInjector;
            injector.inject(text)
        }
        TextInputMethod::Type => {
            let injector = TypeInjector;
            injector.inject(text)
        }
    }
}
