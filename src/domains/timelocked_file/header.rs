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
    pub payload_plaintext_bytes: u64,
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
                key_wrap: "blake3-xor-v1".to_string(),
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
            payload_plaintext_bytes: body.payload_plaintext_bytes,
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
