//! Provides a generic string wrapper for sensitive values that redacts debug output
//! and zeroizes its backing memory when cleared or dropped.

use std::fmt;

use zeroize::{Zeroize, Zeroizing};

#[derive(Clone, Default, Eq, PartialEq)]
pub struct SecretString {
    inner: Zeroizing<String>,
}

impl SecretString {
    pub fn new(secret: String) -> Self {
        Self {
            inner: Zeroizing::new(secret),
        }
    }

    pub fn expose_secret(&self) -> &str {
        self.inner.as_str()
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.inner.as_bytes()
    }

    pub fn clear(&mut self) {
        self.inner.zeroize();
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretString([REDACTED])")
    }
}

#[cfg(test)]
mod tests {
    use super::SecretString;

    #[test]
    fn secret_string_debug_is_redacted() {
        let secret = SecretString::new("correct horse battery staple".to_string());

        let debug = format!("{secret:?}");

        assert!(debug.contains("REDACTED"));
        assert!(!debug.contains("correct"));
        assert!(!debug.contains("horse"));
        assert!(!debug.contains("battery"));
        assert!(!debug.contains("staple"));
    }

    #[test]
    fn secret_string_zeroizes_on_clear() {
        let original = "sensitive passphrase";
        let mut secret = SecretString::new(original.to_string());
        let ptr = secret.inner.as_ptr();
        let len = secret.inner.len();
        assert_eq!(len, original.len());

        secret.clear();

        assert_eq!(secret.expose_secret(), "");
        let bytes_after_clear = unsafe { std::slice::from_raw_parts(ptr, len) };
        assert!(bytes_after_clear.iter().all(|byte| *byte == 0));
    }
}
