//! Holds editable TUI input state for sensitive text values.
//!
//! This wraps the regular text field editing behavior while redacting debug output
//! and making a best-effort attempt to zeroize the backing string when cleared or
//! dropped. Rendering code should use only the exposed character count for masks.

use std::fmt;

use crossterm::event::KeyEvent;

use super::TextField;

#[derive(Clone)]
pub struct SecretTextField {
    field: TextField,
}

impl SecretTextField {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            field: TextField::new(value),
        }
    }

    pub fn apply_key(&mut self, key: KeyEvent) -> bool {
        self.field.apply_key(key)
    }

    pub(crate) fn arm_clear_on_next_edit(&mut self) {
        self.field.arm_clear_on_next_edit();
    }

    pub fn expose_secret(&self) -> &str {
        &self.field.value
    }

    pub fn secret_char_count(&self) -> usize {
        self.field.value.chars().count()
    }

    pub fn is_empty(&self) -> bool {
        self.field.value.is_empty()
    }

    pub fn clear(&mut self) {
        self.field.clear();
    }
}

impl Default for SecretTextField {
    fn default() -> Self {
        Self::new(String::new())
    }
}

impl fmt::Debug for SecretTextField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretTextField([REDACTED])")
    }
}

impl Drop for SecretTextField {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::SecretTextField;

    #[test]
    fn debug_redacts_secret_value() {
        let field = SecretTextField::new("correct horse".to_string());

        let debug = format!("{field:?}");

        assert!(debug.contains("REDACTED"));
        assert!(!debug.contains("correct"));
        assert!(!debug.contains("horse"));
    }

    #[test]
    fn delegates_text_editing() {
        let mut field = SecretTextField::new("ab".to_string());

        assert!(field.apply_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE)));
        assert!(field.apply_key(KeyEvent::new(KeyCode::Char('X'), KeyModifiers::NONE)));

        assert_eq!(field.expose_secret(), "aXb");
    }

    #[test]
    fn clear_zeroizes_secret_value() {
        let original = "sensitive passphrase";
        let mut field = SecretTextField::new(original.to_string());
        let ptr = field.field.value.as_ptr();
        let len = field.field.value.len();
        assert_eq!(len, original.len());

        field.clear();

        assert_eq!(field.expose_secret(), "");
        let bytes_after_clear = unsafe { std::slice::from_raw_parts(ptr, len) };
        assert!(bytes_after_clear.iter().all(|byte| *byte == 0));
    }
}
