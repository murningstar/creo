pub mod injector;
pub mod paste;
pub mod typer;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextInputMethod {
    Paste,
    Type,
}

impl TextInputMethod {
    pub fn from_str_lossy(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "type" => TextInputMethod::Type,
            _ => TextInputMethod::Paste,
        }
    }
}
