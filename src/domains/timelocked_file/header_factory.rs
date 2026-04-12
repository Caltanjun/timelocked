//! Factory for constructing valid header structures during the locking process.
//! Fills in default algorithms, chunk sizes, and timestamps.

use chrono::Utc;

use super::{ChunkingParams, CipherParams, TimelockParams, TimelockedHeader};

pub const DEFAULT_LOCK_CHUNK_SIZE_BYTES: usize = 1024 * 1024;

const ABOUT_TEXT: &str = "This is a Timelocked file. Use the Timelocked app to unlock the original content after the required compute delay. Tools and docs: https://timelocked.app";

#[derive(Debug, Clone)]
pub struct LockHeaderSpec {
    pub original_filename: Option<String>,
    pub creator_name: Option<String>,
    pub creator_message: Option<String>,
    pub iterations: u64,
    pub modulus_bits: u16,
    pub hardware_profile: String,
    pub target_seconds: Option<u64>,
    pub payload_plaintext_bytes: u64,
}

pub fn build_lock_header(spec: LockHeaderSpec) -> TimelockedHeader {
    TimelockedHeader {
        about: ABOUT_TEXT.to_string(),
        original_filename: spec.original_filename,
        created_at: Utc::now().to_rfc3339(),
        creator_name: spec.creator_name,
        creator_message: spec.creator_message,
        cipher_params: CipherParams {
            payload_cipher: "XChaCha20-Poly1305".to_string(),
            key_wrap: "blake3-xor-v1".to_string(),
        },
        chunking_params: ChunkingParams {
            chunk_size_bytes: DEFAULT_LOCK_CHUNK_SIZE_BYTES as u32,
        },
        timelock_params: TimelockParams {
            algorithm: "rsw-repeated-squaring-v1".to_string(),
            iterations: spec.iterations,
            modulus_bits: spec.modulus_bits,
            hardware_profile: spec.hardware_profile,
            target_seconds: spec.target_seconds,
        },
        payload_plaintext_bytes: spec.payload_plaintext_bytes,
    }
}

#[cfg(test)]
mod tests {
    use super::{build_lock_header, LockHeaderSpec, DEFAULT_LOCK_CHUNK_SIZE_BYTES};

    #[test]
    fn build_lock_header_sets_expected_domain_fields() {
        let header = build_lock_header(LockHeaderSpec {
            original_filename: Some("letter.txt".to_string()),
            creator_name: Some("Creator".to_string()),
            creator_message: Some("hello".to_string()),
            iterations: 123,
            modulus_bits: 512,
            hardware_profile: "desktop-2026".to_string(),
            target_seconds: Some(60),
            payload_plaintext_bytes: 42,
        });

        assert_eq!(header.original_filename.as_deref(), Some("letter.txt"));
        assert_eq!(header.timelock_params.iterations, 123);
        assert_eq!(header.timelock_params.hardware_profile, "desktop-2026");
        assert_eq!(header.timelock_params.target_seconds, Some(60));
        assert_eq!(header.payload_plaintext_bytes, 42);
        assert_eq!(
            header.chunking_params.chunk_size_bytes,
            DEFAULT_LOCK_CHUNK_SIZE_BYTES as u32
        );
    }
}
