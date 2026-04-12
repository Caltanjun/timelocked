//! Unlock-side recovery from stored artifacts.

use std::io::{Cursor, Write};
use std::path::Path;

use zeroize::Zeroize;

use crate::base::progress_status::ProgressStatus;
use crate::base::{CancellationToken, Result};

use super::payload_region::PayloadRegionLayout;
use super::reader::read_payload_region_bytes;
use super::{
    decrypt_protected_stream_to_writer_with_cancel, reconstruct_payload_region, superblock_digest,
    ChunkDecryptionStats, ParsedContainer, TimelockPayloadMaterial,
};

#[derive(Debug, Clone, Copy)]
pub struct RecoverFileKeyRequest<'a> {
    pub material: &'a TimelockPayloadMaterial,
    pub iterations: u64,
    pub modulus_bits: u16,
}

pub type RecoverFileKeyFn = dyn FnMut(
    RecoverFileKeyRequest<'_>,
    Option<&mut dyn FnMut(ProgressStatus)>,
    Option<&CancellationToken>,
) -> Result<[u8; 32]>;

pub fn recover_protected_stream_to_writer_with_cancel(
    path: &Path,
    parsed: &ParsedContainer,
    writer: &mut impl Write,
    cancellation: Option<&CancellationToken>,
) -> Result<()> {
    let payload_region = read_payload_region_bytes(path, parsed)?;
    let protected_stream = reconstruct_payload_region(
        &payload_region,
        parsed.superblock.protected_stream_len,
        PayloadRegionLayout {
            rs_data_shards: parsed.superblock.rs_data_shards,
            rs_parity_shards: parsed.superblock.rs_parity_shards,
            rs_shard_bytes: parsed.superblock.rs_shard_bytes,
        },
    )?;

    crate::base::ensure_not_cancelled(cancellation)?;
    writer.write_all(&protected_stream)?;
    Ok(())
}

pub fn recover_payload_to_writer_with_cancel(
    path: &Path,
    parsed: &ParsedContainer,
    writer: &mut impl Write,
    recover_file_key: &mut RecoverFileKeyFn,
    mut on_progress: Option<&mut dyn FnMut(ProgressStatus)>,
    cancellation: Option<&CancellationToken>,
) -> Result<ChunkDecryptionStats> {
    let recover_request = RecoverFileKeyRequest {
        material: &parsed.superblock.timelock_material,
        iterations: parsed.superblock.iterations,
        modulus_bits: parsed.superblock.modulus_bits,
    };
    let mut key = if let Some(progress) = on_progress.as_mut() {
        recover_file_key(recover_request, Some(&mut **progress), cancellation)?
    } else {
        recover_file_key(recover_request, None, cancellation)?
    };

    let payload_region = read_payload_region_bytes(path, parsed)?;
    let protected_stream = reconstruct_payload_region(
        &payload_region,
        parsed.superblock.protected_stream_len,
        PayloadRegionLayout {
            rs_data_shards: parsed.superblock.rs_data_shards,
            rs_parity_shards: parsed.superblock.rs_parity_shards,
            rs_shard_bytes: parsed.superblock.rs_shard_bytes,
        },
    )?;
    let digest = superblock_digest(&parsed.superblock)?;

    let decrypt_result = if let Some(progress) = on_progress.as_mut() {
        decrypt_protected_stream_to_writer_with_cancel(
            &mut Cursor::new(protected_stream),
            writer,
            &key,
            &digest,
            parsed.superblock.aead_chunk_size_bytes,
            parsed.superblock.payload_plaintext_bytes,
            Some(&mut **progress),
            cancellation,
        )
    } else {
        decrypt_protected_stream_to_writer_with_cancel(
            &mut Cursor::new(protected_stream),
            writer,
            &key,
            &digest,
            parsed.superblock.aead_chunk_size_bytes,
            parsed.superblock.payload_plaintext_bytes,
            None,
            cancellation,
        )
    };
    key.zeroize();
    decrypt_result
}
