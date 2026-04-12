//! Full file writer for the new Timelocked binary artifact.

use std::io::{Read, Write};

use crate::base::progress_status::ProgressStatus;
use crate::base::{CancellationToken, Error, Result};

use super::notice::{encode_notice, DEFAULT_NOTICE_TEXT};
use super::payload_region::{
    encode_payload_region, encoded_payload_region_len, PayloadRegionLayout,
};
use super::superblock::{encode_end_superblock_copy, encode_start_superblock_copy};
use super::{
    encrypt_protected_stream_with_cancel, superblock_digest, SuperblockBody,
    TimelockPayloadMaterial, AEAD_CIPHER_ID_XCHACHA20POLY1305, BODY_VERSION_V1,
    RS_ALGORITHM_ID_GF256_REED_SOLOMON, TIMELOCK_ALGORITHM_ID_RSW_REPEATED_SQUARING_V1,
};

pub const DEFAULT_LOCK_CHUNK_SIZE_BYTES: usize = 1024 * 1024;
pub const DEFAULT_RS_DATA_SHARDS: u16 = 4;
pub const DEFAULT_RS_PARITY_SHARDS: u16 = 2;
pub const DEFAULT_RS_SHARD_BYTES: u32 = 64 * 1024;
pub const MIN_ADAPTIVE_RS_SHARD_BYTES: u32 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PayloadRegionEncodingParams {
    pub rs_data_shards: u16,
    pub rs_parity_shards: u16,
    pub rs_shard_bytes: u32,
}

#[derive(Debug, Clone)]
pub struct LockArtifactRequest {
    pub notice: Option<String>,
    pub original_filename: Option<String>,
    pub iterations: u64,
    pub modulus_bits: u16,
    pub hardware_profile: String,
    pub target_seconds: Option<u64>,
    pub created_at_unix_seconds: u64,
    pub payload_plaintext_bytes: u64,
    pub aead_chunk_size_bytes: u32,
    pub payload_region_params: PayloadRegionEncodingParams,
    pub key_bytes: [u8; 32],
    pub timelock_material: TimelockPayloadMaterial,
}

pub fn write_timelocked_artifact(
    writer: &mut impl Write,
    plaintext_reader: &mut impl Read,
    request: &LockArtifactRequest,
    on_progress: Option<&mut dyn FnMut(ProgressStatus)>,
    cancellation: Option<&CancellationToken>,
) -> Result<()> {
    let layout = PayloadRegionLayout {
        rs_data_shards: request.payload_region_params.rs_data_shards,
        rs_parity_shards: request.payload_region_params.rs_parity_shards,
        rs_shard_bytes: request.payload_region_params.rs_shard_bytes,
    };

    let protected_stream_len = predicted_protected_stream_len(
        request.payload_plaintext_bytes,
        request.aead_chunk_size_bytes,
    )?;
    let payload_region_len = encoded_payload_region_len(protected_stream_len, layout)?;

    let superblock = SuperblockBody {
        body_version: BODY_VERSION_V1,
        flags: 0,
        payload_plaintext_bytes: request.payload_plaintext_bytes,
        protected_stream_len,
        payload_region_len,
        aead_chunk_size_bytes: request.aead_chunk_size_bytes,
        aead_cipher_id: AEAD_CIPHER_ID_XCHACHA20POLY1305,
        rs_algorithm_id: RS_ALGORITHM_ID_GF256_REED_SOLOMON,
        rs_data_shards: layout.rs_data_shards,
        rs_parity_shards: layout.rs_parity_shards,
        rs_shard_bytes: layout.rs_shard_bytes,
        timelock_algorithm_id: TIMELOCK_ALGORITHM_ID_RSW_REPEATED_SQUARING_V1,
        iterations: request.iterations,
        modulus_bits: request.modulus_bits,
        target_seconds: request.target_seconds,
        created_at_unix_seconds: request.created_at_unix_seconds,
        original_filename: request.original_filename.clone(),
        hardware_profile: request.hardware_profile.clone(),
        timelock_material: request.timelock_material.clone(),
    };

    let digest = superblock_digest(&superblock)?;
    let mut protected_stream = Vec::new();
    let stats = encrypt_protected_stream_with_cancel(
        plaintext_reader,
        &mut protected_stream,
        &request.key_bytes,
        &digest,
        request.aead_chunk_size_bytes as usize,
        request.payload_plaintext_bytes,
        on_progress,
        cancellation,
    )?;
    if stats.plaintext_bytes != request.payload_plaintext_bytes {
        return Err(Error::InvalidFormat(
            "protected stream plaintext length mismatch".to_string(),
        ));
    }
    if protected_stream.len() as u64 != superblock.protected_stream_len {
        return Err(Error::InvalidFormat(
            "protected stream length mismatch".to_string(),
        ));
    }

    let payload_region = encode_payload_region(&protected_stream, layout)?;
    if payload_region.len() as u64 != superblock.payload_region_len {
        return Err(Error::InvalidFormat(
            "payload region length mismatch".to_string(),
        ));
    }

    let notice = request.notice.as_deref().unwrap_or(DEFAULT_NOTICE_TEXT);
    writer.write_all(&encode_notice(notice)?)?;
    writer.write_all(&encode_start_superblock_copy(&superblock)?)?;
    writer.write_all(&payload_region)?;
    writer.write_all(&encode_end_superblock_copy(&superblock)?)?;
    Ok(())
}

pub fn predicted_protected_stream_len(
    payload_plaintext_bytes: u64,
    chunk_size: u32,
) -> Result<u64> {
    if chunk_size == 0 {
        return Err(Error::InvalidArgument(
            "aead_chunk_size_bytes must be greater than 0".to_string(),
        ));
    }
    let chunk_count = if payload_plaintext_bytes == 0 {
        1
    } else {
        payload_plaintext_bytes.div_ceil(chunk_size as u64)
    };
    Ok(payload_plaintext_bytes + (chunk_count * 57))
}

pub fn choose_rs_shard_bytes_for_protected_stream_len(protected_stream_len: u64) -> u32 {
    if protected_stream_len >= u64::from(DEFAULT_RS_SHARD_BYTES) {
        return DEFAULT_RS_SHARD_BYTES;
    }

    let required_shard_bytes = protected_stream_len.div_ceil(u64::from(DEFAULT_RS_DATA_SHARDS));
    let bucketed_shard_bytes = required_shard_bytes.next_power_of_two();

    bucketed_shard_bytes.max(u64::from(MIN_ADAPTIVE_RS_SHARD_BYTES)) as u32
}

#[cfg(test)]
mod tests {
    use super::{
        choose_rs_shard_bytes_for_protected_stream_len, DEFAULT_RS_DATA_SHARDS,
        DEFAULT_RS_SHARD_BYTES, MIN_ADAPTIVE_RS_SHARD_BYTES,
    };

    #[test]
    fn adaptive_rs_shard_bytes_uses_minimum_floor_for_empty_stream() {
        assert_eq!(
            choose_rs_shard_bytes_for_protected_stream_len(0),
            MIN_ADAPTIVE_RS_SHARD_BYTES
        );
    }

    #[test]
    fn adaptive_rs_shard_bytes_uses_power_of_two_buckets_for_small_streams() {
        assert_eq!(
            choose_rs_shard_bytes_for_protected_stream_len(1),
            MIN_ADAPTIVE_RS_SHARD_BYTES
        );
        assert_eq!(
            choose_rs_shard_bytes_for_protected_stream_len(u64::from(DEFAULT_RS_DATA_SHARDS) * 64),
            64
        );
        assert_eq!(choose_rs_shard_bytes_for_protected_stream_len(257), 128);
        assert_eq!(choose_rs_shard_bytes_for_protected_stream_len(513), 256);
        assert_eq!(choose_rs_shard_bytes_for_protected_stream_len(4097), 2048);
    }

    #[test]
    fn adaptive_rs_shard_bytes_keeps_large_streams_at_64_kib() {
        assert_eq!(
            choose_rs_shard_bytes_for_protected_stream_len(u64::from(DEFAULT_RS_SHARD_BYTES) - 1),
            16 * 1024
        );
        assert_eq!(
            choose_rs_shard_bytes_for_protected_stream_len(u64::from(DEFAULT_RS_SHARD_BYTES)),
            DEFAULT_RS_SHARD_BYTES
        );
        assert_eq!(
            choose_rs_shard_bytes_for_protected_stream_len(u64::from(DEFAULT_RS_SHARD_BYTES) + 1),
            DEFAULT_RS_SHARD_BYTES
        );
    }
}
