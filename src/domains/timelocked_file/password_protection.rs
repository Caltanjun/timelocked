//! Password-based key protection for `.timelocked` file keys.
//! Owns Argon2id parameter validation, wrap-key derivation, and reversible
//! file-key wrapping used by future password-protected file-format versions.

use argon2::{Algorithm, Argon2, Params, Version};
use zeroize::{Zeroize, Zeroizing};

use crate::base::{Error, Result, SecretString};

const PASSWORD_WRAP_DOMAIN_SEPARATOR: &[u8] = b"TLCK-PASSWORD-WRAP-v1";
const PASSWORD_WRAP_KEY_SIZE: usize = 32;

pub const KEY_PROTECTION_ALGORITHM_ID_TIMELOCK_ONLY: u8 = 1;
pub const KEY_PROTECTION_ALGORITHM_ID_TIMELOCK_PLUS_ARGON2ID_V1: u8 = 2;
pub const PASSWORD_KDF_ALGORITHM_ID_ARGON2ID: u8 = 1;

pub const DEFAULT_ARGON2ID_MEMORY_KIB: u32 = 65_536;
pub const DEFAULT_ARGON2ID_ITERATIONS: u32 = 3;
pub const DEFAULT_ARGON2ID_PARALLELISM: u32 = 1;
pub const DEFAULT_PASSWORD_SALT_LEN: usize = 32;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum KeyProtectionAlgorithm {
    TimelockOnly,
    TimelockPlusArgon2idV1,
}

impl KeyProtectionAlgorithm {
    pub fn stable_id(self) -> u8 {
        match self {
            Self::TimelockOnly => KEY_PROTECTION_ALGORITHM_ID_TIMELOCK_ONLY,
            Self::TimelockPlusArgon2idV1 => KEY_PROTECTION_ALGORITHM_ID_TIMELOCK_PLUS_ARGON2ID_V1,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PasswordKdfAlgorithm {
    Argon2id,
}

impl PasswordKdfAlgorithm {
    pub fn stable_id(self) -> u8 {
        match self {
            Self::Argon2id => PASSWORD_KDF_ALGORITHM_ID_ARGON2ID,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PasswordProtectionParams {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub salt: Vec<u8>,
}

impl PasswordProtectionParams {
    pub fn new(memory_kib: u32, iterations: u32, parallelism: u32, salt: Vec<u8>) -> Result<Self> {
        let params = Self {
            memory_kib,
            iterations,
            parallelism,
            salt,
        };
        params.validate()?;
        Ok(params)
    }

    pub fn default_with_salt(salt: Vec<u8>) -> Result<Self> {
        Self::new(
            DEFAULT_ARGON2ID_MEMORY_KIB,
            DEFAULT_ARGON2ID_ITERATIONS,
            DEFAULT_ARGON2ID_PARALLELISM,
            salt,
        )
    }

    pub fn validate(&self) -> Result<()> {
        if self.salt.is_empty() {
            return Err(Error::InvalidArgument(
                "password protection salt must not be empty".to_string(),
            ));
        }
        if self.memory_kib == 0 {
            return Err(Error::InvalidArgument(
                "password KDF memory cost must be greater than zero".to_string(),
            ));
        }
        if self.iterations == 0 {
            return Err(Error::InvalidArgument(
                "password KDF iterations must be greater than zero".to_string(),
            ));
        }
        if self.parallelism == 0 {
            return Err(Error::InvalidArgument(
                "password KDF parallelism must be greater than zero".to_string(),
            ));
        }
        Ok(())
    }
}

pub fn derive_password_wrap_key(
    params: &PasswordProtectionParams,
    timelock_mask: &[u8; PASSWORD_WRAP_KEY_SIZE],
    passphrase: &SecretString,
) -> Result<[u8; PASSWORD_WRAP_KEY_SIZE]> {
    params.validate()?;

    let argon2_params = Params::new(
        params.memory_kib,
        params.iterations,
        params.parallelism,
        Some(PASSWORD_WRAP_KEY_SIZE),
    )
    .map_err(|err| Error::InvalidArgument(format!("invalid Argon2id parameters: {err}")))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon2_params);

    let mut kdf_input = Zeroizing::new(Vec::with_capacity(
        PASSWORD_WRAP_DOMAIN_SEPARATOR.len() + timelock_mask.len() + passphrase.as_bytes().len(),
    ));
    kdf_input.extend_from_slice(PASSWORD_WRAP_DOMAIN_SEPARATOR);
    kdf_input.extend_from_slice(timelock_mask);
    kdf_input.extend_from_slice(passphrase.as_bytes());

    let mut wrap_key = [0u8; PASSWORD_WRAP_KEY_SIZE];
    argon2
        .hash_password_into(&kdf_input, &params.salt, &mut wrap_key)
        .map_err(|err| Error::Crypto(format!("password KDF failed: {err}")))?;

    Ok(wrap_key)
}

pub fn wrap_file_key_with_password(
    file_key: &[u8; PASSWORD_WRAP_KEY_SIZE],
    timelock_mask: &[u8; PASSWORD_WRAP_KEY_SIZE],
    passphrase: &SecretString,
    params: &PasswordProtectionParams,
) -> Result<[u8; PASSWORD_WRAP_KEY_SIZE]> {
    let mut wrap_key = derive_password_wrap_key(params, timelock_mask, passphrase)?;
    let wrapped_key = xor_32(file_key, &wrap_key);
    wrap_key.zeroize();
    Ok(wrapped_key)
}

pub fn wrap_file_key_with_timelock_mask(
    file_key: &[u8; PASSWORD_WRAP_KEY_SIZE],
    timelock_mask: &[u8; PASSWORD_WRAP_KEY_SIZE],
) -> [u8; PASSWORD_WRAP_KEY_SIZE] {
    xor_32(file_key, timelock_mask)
}

pub fn validate_lock_passphrase(passphrase: &SecretString) -> Result<()> {
    if passphrase.as_bytes().is_empty() {
        return Err(Error::InvalidArgument(
            "password must not be empty".to_string(),
        ));
    }

    Ok(())
}

pub fn wrap_file_key_for_password_protected_lock(
    file_key: &[u8; PASSWORD_WRAP_KEY_SIZE],
    timelock_mask: &[u8; PASSWORD_WRAP_KEY_SIZE],
    passphrase: &SecretString,
    params: PasswordProtectionParams,
) -> Result<(
    [u8; PASSWORD_WRAP_KEY_SIZE],
    super::PasswordProtectionMetadata,
)> {
    validate_lock_passphrase(passphrase)?;

    let wrapped_key = wrap_file_key_with_password(file_key, timelock_mask, passphrase, &params)?;
    let metadata = super::PasswordProtectionMetadata::timelock_plus_argon2id_v1(params);
    Ok((wrapped_key, metadata))
}

pub fn unwrap_file_key_with_password(
    wrapped_key: &[u8; PASSWORD_WRAP_KEY_SIZE],
    timelock_mask: &[u8; PASSWORD_WRAP_KEY_SIZE],
    passphrase: &SecretString,
    params: &PasswordProtectionParams,
) -> Result<[u8; PASSWORD_WRAP_KEY_SIZE]> {
    let mut wrap_key = derive_password_wrap_key(params, timelock_mask, passphrase)?;
    let file_key = xor_32(wrapped_key, &wrap_key);
    wrap_key.zeroize();
    Ok(file_key)
}

fn xor_32(
    left: &[u8; PASSWORD_WRAP_KEY_SIZE],
    right: &[u8; PASSWORD_WRAP_KEY_SIZE],
) -> [u8; PASSWORD_WRAP_KEY_SIZE] {
    let mut output = [0u8; PASSWORD_WRAP_KEY_SIZE];
    for ((out, left), right) in output.iter_mut().zip(left.iter()).zip(right.iter()) {
        *out = left ^ right;
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_params_with_salt(salt: Vec<u8>) -> PasswordProtectionParams {
        PasswordProtectionParams::new(8, 1, 1, salt).expect("test params should be valid")
    }

    fn test_passphrase(value: &str) -> SecretString {
        SecretString::new(value.to_string())
    }

    #[test]
    fn password_params_reject_empty_salt() {
        let err = PasswordProtectionParams::new(8, 1, 1, Vec::new()).unwrap_err();

        assert!(matches!(err, Error::InvalidArgument(_)));
    }

    #[test]
    fn password_params_reject_zero_memory_cost() {
        let err = PasswordProtectionParams::new(0, 1, 1, vec![1; 16]).unwrap_err();

        assert!(matches!(err, Error::InvalidArgument(_)));
    }

    #[test]
    fn password_wrap_key_is_deterministic_for_same_inputs() {
        let params = test_params_with_salt(vec![7; 16]);
        let timelock_mask = [42u8; 32];
        let passphrase = test_passphrase("correct horse battery staple");

        let first = derive_password_wrap_key(&params, &timelock_mask, &passphrase).unwrap();
        let second = derive_password_wrap_key(&params, &timelock_mask, &passphrase).unwrap();

        assert_eq!(first, second);
    }

    #[test]
    fn password_wrap_key_changes_when_passphrase_changes() {
        let params = test_params_with_salt(vec![7; 16]);
        let timelock_mask = [42u8; 32];

        let first =
            derive_password_wrap_key(&params, &timelock_mask, &test_passphrase("one")).unwrap();
        let second =
            derive_password_wrap_key(&params, &timelock_mask, &test_passphrase("two")).unwrap();

        assert_ne!(first, second);
    }

    #[test]
    fn password_wrap_key_changes_when_timelock_mask_changes() {
        let params = test_params_with_salt(vec![7; 16]);
        let passphrase = test_passphrase("correct horse battery staple");
        let first_mask = [42u8; 32];
        let second_mask = [43u8; 32];

        let first = derive_password_wrap_key(&params, &first_mask, &passphrase).unwrap();
        let second = derive_password_wrap_key(&params, &second_mask, &passphrase).unwrap();

        assert_ne!(first, second);
    }

    #[test]
    fn password_wrap_key_changes_when_salt_changes() {
        let first_params = test_params_with_salt(vec![7; 16]);
        let second_params = test_params_with_salt(vec![8; 16]);
        let timelock_mask = [42u8; 32];
        let passphrase = test_passphrase("correct horse battery staple");

        let first = derive_password_wrap_key(&first_params, &timelock_mask, &passphrase).unwrap();
        let second = derive_password_wrap_key(&second_params, &timelock_mask, &passphrase).unwrap();

        assert_ne!(first, second);
    }

    #[test]
    fn password_wrap_and_unwrap_file_key_round_trip() {
        let params = test_params_with_salt(vec![7; 16]);
        let timelock_mask = [42u8; 32];
        let passphrase = test_passphrase("correct horse battery staple");
        let file_key = [99u8; 32];

        let wrapped_key =
            wrap_file_key_with_password(&file_key, &timelock_mask, &passphrase, &params).unwrap();
        let unwrapped_key =
            unwrap_file_key_with_password(&wrapped_key, &timelock_mask, &passphrase, &params)
                .unwrap();

        assert_eq!(unwrapped_key, file_key);
    }

    #[test]
    fn wrong_password_unwraps_to_different_file_key() {
        let params = test_params_with_salt(vec![7; 16]);
        let timelock_mask = [42u8; 32];
        let file_key = [99u8; 32];

        let wrapped_key = wrap_file_key_with_password(
            &file_key,
            &timelock_mask,
            &test_passphrase("correct password"),
            &params,
        )
        .unwrap();
        let unwrapped_key = unwrap_file_key_with_password(
            &wrapped_key,
            &timelock_mask,
            &test_passphrase("wrong password"),
            &params,
        )
        .unwrap();

        assert_ne!(unwrapped_key, file_key);
    }
}
