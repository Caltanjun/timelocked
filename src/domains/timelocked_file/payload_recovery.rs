//! Recovers payload bytes from a `.timelocked` container once a file key is resolved.
//! The timelock-solving step is injected so this domain stays focused on file rules.

use std::io::{Read, Write};

use zeroize::Zeroize;

use crate::base::progress_status::ProgressStatus;
use crate::base::{CancellationToken, Result};

use super::{
    compute_header_digest, decrypt_payload_to_writer_with_cancel, ChunkDecryptionStats,
    ParsedContainer, TimelockPayloadMaterial,
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

pub fn recover_payload_to_writer_with_cancel(
    payload_reader: &mut impl Read,
    parsed: &ParsedContainer,
    writer: &mut impl Write,
    recover_file_key: &mut RecoverFileKeyFn,
    mut on_progress: Option<&mut dyn FnMut(ProgressStatus)>,
    cancellation: Option<&CancellationToken>,
) -> Result<ChunkDecryptionStats> {
    let material = super::read_timelock_material(payload_reader)?;
    let recover_request = RecoverFileKeyRequest {
        material: &material,
        iterations: parsed.header.timelock_params.iterations,
        modulus_bits: parsed.header.timelock_params.modulus_bits,
    };

    let mut key = if let Some(progress) = on_progress.as_mut() {
        recover_file_key(recover_request, Some(&mut **progress), cancellation)?
    } else {
        recover_file_key(recover_request, None, cancellation)?
    };
    let header_digest = compute_header_digest(&parsed.header_bytes);

    let decrypt_result = if let Some(progress) = on_progress.as_mut() {
        decrypt_payload_to_writer_with_cancel(
            payload_reader,
            writer,
            &key,
            &header_digest,
            parsed.header.chunking_params.chunk_size_bytes,
            parsed.header.payload_plaintext_bytes,
            Some(&mut **progress),
            cancellation,
        )
    } else {
        decrypt_payload_to_writer_with_cancel(
            payload_reader,
            writer,
            &key,
            &header_digest,
            parsed.header.chunking_params.chunk_size_bytes,
            parsed.header.payload_plaintext_bytes,
            None,
            cancellation,
        )
    };
    key.zeroize();

    decrypt_result
}
