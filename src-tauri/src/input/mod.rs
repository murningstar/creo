pub mod injector;
pub mod paste;
pub mod typer;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextInputMethod {
    #[serde(rename = "paste")]
    Paste,
    #[serde(rename = "type")]
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Pinned wire format for TextInputMethod — if this fails, you changed a serialized value.
    #[test]
    fn text_input_method_serialization_stability() {
        assert_eq!(serde_json::to_string(&TextInputMethod::Paste).unwrap(), "\"paste\"");
        assert_eq!(serde_json::to_string(&TextInputMethod::Type).unwrap(), "\"type\"");
    }
}
