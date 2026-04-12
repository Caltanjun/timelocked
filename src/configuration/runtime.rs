//! Resolves runtime configuration values from defaults and environment overrides.

const DEFAULT_LOCK_MODULUS_BITS: usize = 2048;

pub fn lock_modulus_bits() -> usize {
    resolve_lock_modulus_bits(std::env::var("MODULUS_BITS").ok().as_deref())
}

fn resolve_lock_modulus_bits(env_value: Option<&str>) -> usize {
    env_value
        .and_then(|value| value.parse().ok())
        .unwrap_or(default_lock_modulus_bits())
}

fn default_lock_modulus_bits() -> usize {
    if cfg!(test) {
        256
    } else {
        DEFAULT_LOCK_MODULUS_BITS
    }
}

#[cfg(test)]
mod tests {
    use super::{default_lock_modulus_bits, resolve_lock_modulus_bits};

    #[test]
    fn resolves_default_modulus_bits_without_override() {
        assert_eq!(resolve_lock_modulus_bits(None), default_lock_modulus_bits());
    }

    #[test]
    fn resolves_override_when_env_value_is_valid() {
        assert_eq!(resolve_lock_modulus_bits(Some("512")), 512);
    }

    #[test]
    fn ignores_invalid_override_values() {
        assert_eq!(
            resolve_lock_modulus_bits(Some("not-a-number")),
            default_lock_modulus_bits()
        );
        assert_eq!(
            resolve_lock_modulus_bits(Some("")),
            default_lock_modulus_bits()
        );
    }
}
