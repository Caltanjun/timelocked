//! Inspect-facing metadata derived from the authoritative superblock.
//! This keeps UI code independent from binary layout details.

use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};

use super::superblock_body::SuperblockBody;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimelockedHeader {
    pub about: String,
    pub original_filename: Option<String>,
    pub created_at: String,
    pub creator_name: Option<String>,
    pub creator_message: Option<String>,
    pub cipher_params: CipherParams,
    pub chunking_params: ChunkingParams,
    pub timelock_params: TimelockParams,
    pub password_protection: PasswordProtectionSummary,
    pub payload_plaintext_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PasswordProtectionSummary {
    pub password_protected: bool,
    pub key_protection_algorithm: Option<String>,
    pub password_kdf_algorithm: Option<String>,
    pub password_kdf_memory_kib: Option<u32>,
    pub password_kdf_iterations: Option<u32>,
    pub password_kdf_parallelism: Option<u32>,
    pub password_salt_len: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CipherParams {
    pub payload_cipher: String,
    pub key_wrap: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChunkingParams {
    pub chunk_size_bytes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimelockParams {
    pub algorithm: String,
    pub iterations: u64,
    pub modulus_bits: u16,
    pub hardware_profile: String,
    pub target_seconds: Option<u64>,
}

impl TimelockedHeader {
    pub(crate) fn from_superblock(body: &SuperblockBody, notice: &str) -> Self {
        Self {
            about: notice.to_string(),
            original_filename: body.original_filename.clone(),
            created_at: format_created_at(body.created_at_unix_seconds),
            creator_name: None,
            creator_message: None,
            cipher_params: CipherParams {
                payload_cipher: "XChaCha20-Poly1305".to_string(),
                key_wrap: inspect_key_wrap_algorithm(body).to_string(),
            },
            chunking_params: ChunkingParams {
                chunk_size_bytes: body.aead_chunk_size_bytes,
            },
            timelock_params: TimelockParams {
                algorithm: "rsw-repeated-squaring-v1".to_string(),
                iterations: body.iterations,
                modulus_bits: body.modulus_bits,
                hardware_profile: body.hardware_profile.clone(),
                target_seconds: body.target_seconds,
            },
            password_protection: password_protection_summary(body),
            payload_plaintext_bytes: body.payload_plaintext_bytes,
        }
    }
}

fn inspect_key_wrap_algorithm(body: &SuperblockBody) -> &'static str {
    if body.password_protection.is_some() {
        "timelock-plus-argon2id-v1"
    } else {
        "blake3-xor-v1"
    }
}

fn password_protection_summary(body: &SuperblockBody) -> PasswordProtectionSummary {
    match &body.password_protection {
        Some(metadata) => PasswordProtectionSummary {
            password_protected: true,
            key_protection_algorithm: Some("timelock-plus-argon2id-v1".to_string()),
            password_kdf_algorithm: Some("argon2id".to_string()),
            password_kdf_memory_kib: Some(metadata.params.memory_kib),
            password_kdf_iterations: Some(metadata.params.iterations),
            password_kdf_parallelism: Some(metadata.params.parallelism),
            password_salt_len: Some(metadata.params.salt.len()),
        },
        None => PasswordProtectionSummary {
            password_protected: false,
            key_protection_algorithm: None,
            password_kdf_algorithm: None,
            password_kdf_memory_kib: None,
            password_kdf_iterations: None,
            password_kdf_parallelism: None,
            password_salt_len: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use num_bigint::BigUint;

    use super::TimelockedHeader;
    use crate::domains::timelocked_file::password_protection::PasswordProtectionParams;
    use crate::domains::timelocked_file::superblock_body::{
        PasswordProtectionMetadata, SuperblockBody, TimelockPayloadMaterial, BODY_VERSION_V1,
        BODY_VERSION_V2,
    };
    use crate::domains::timelocked_file::{
        AEAD_CIPHER_ID_XCHACHA20POLY1305, RS_ALGORITHM_ID_GF256_REED_SOLOMON,
        TIMELOCK_ALGORITHM_ID_RSW_REPEATED_SQUARING_V1,
    };

    #[test]
    fn inspect_header_reports_timelock_key_wrap_for_unprotected_v1_body() {
        let body = sample_body();

        let header = TimelockedHeader::from_superblock(&body, "notice");

        assert_eq!(header.cipher_params.key_wrap, "blake3-xor-v1");
        assert!(!header.password_protection.password_protected);
    }

    #[test]
    fn inspect_header_reports_password_key_wrap_for_protected_v2_body() {
        let mut body = sample_body();
        body.body_version = BODY_VERSION_V2;
        body.password_protection = Some(PasswordProtectionMetadata::timelock_plus_argon2id_v1(
            PasswordProtectionParams::new(64, 2, 1, vec![1, 2, 3, 4]).expect("params"),
        ));

        let header = TimelockedHeader::from_superblock(&body, "notice");

        assert_eq!(header.cipher_params.key_wrap, "timelock-plus-argon2id-v1");
        assert!(header.password_protection.password_protected);
        assert_eq!(
            header
                .password_protection
                .key_protection_algorithm
                .as_deref(),
            Some("timelock-plus-argon2id-v1")
        );
    }

    fn sample_body() -> SuperblockBody {
        SuperblockBody {
            body_version: BODY_VERSION_V1,
            flags: 0,
            payload_plaintext_bytes: 0,
            protected_stream_len: 57,
            payload_region_len: 384,
            aead_chunk_size_bytes: 1024,
            aead_cipher_id: AEAD_CIPHER_ID_XCHACHA20POLY1305,
            rs_algorithm_id: RS_ALGORITHM_ID_GF256_REED_SOLOMON,
            rs_data_shards: 4,
            rs_parity_shards: 2,
            rs_shard_bytes: 64,
            timelock_algorithm_id: TIMELOCK_ALGORITHM_ID_RSW_REPEATED_SQUARING_V1,
            iterations: 1,
            modulus_bits: 256,
            target_seconds: None,
            created_at_unix_seconds: 1_700_000_000,
            original_filename: None,
            hardware_profile: "test-profile".to_string(),
            timelock_material: TimelockPayloadMaterial {
                modulus_n: BigUint::from(3233_u32),
                base_a: BigUint::from(5_u32),
                wrapped_key: [9_u8; 32],
            },
            password_protection: None,
        }
    }
}

fn format_created_at(created_at_unix_seconds: u64) -> String {
    match Utc
        .timestamp_opt(created_at_unix_seconds as i64, 0)
        .single()
    {
        Some(timestamp) => timestamp.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        None => created_at_unix_seconds.to_string(),
    }
}
