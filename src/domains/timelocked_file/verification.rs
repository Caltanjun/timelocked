//! Structural and full verification flows for `.timelocked` artifacts.

use std::io::{sink, Cursor};
use std::path::Path;

use crate::base::{ensure_not_cancelled, CancellationToken, Error, Result};

use super::payload_region::PayloadRegionLayout;
use super::reader::read_payload_region_bytes;
use super::{
    decrypt_protected_stream_to_writer_with_cancel, reconstruct_payload_region,
    scan_protected_stream_structural, superblock_digest, ChunkDecryptionStats, ParsedContainer,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelockedFileVerification {
    pub chunk_count: u64,
    pub payload_plaintext_bytes: u64,
}

pub fn verify_timelocked_file_structural(
    path: &Path,
    parsed: &ParsedContainer,
) -> Result<TimelockedFileVerification> {
    verify_timelocked_file_structural_and_cancel(path, parsed, None)
}

pub fn verify_timelocked_file_structural_and_cancel(
    path: &Path,
    parsed: &ParsedContainer,
    cancellation: Option<&CancellationToken>,
) -> Result<TimelockedFileVerification> {
    let structural = reconstruct_and_scan_protected_stream(path, parsed, cancellation)?.1;
    Ok(TimelockedFileVerification {
        chunk_count: structural.chunk_count,
        payload_plaintext_bytes: structural.plaintext_bytes,
    })
}

pub fn verify_timelocked_file_with_key(
    path: &Path,
    parsed: &ParsedContainer,
    key_bytes: &[u8; 32],
) -> Result<TimelockedFileVerification> {
    verify_timelocked_file_with_key_and_cancel(path, parsed, key_bytes, None)
}

pub fn verify_timelocked_file_with_key_and_cancel(
    path: &Path,
    parsed: &ParsedContainer,
    key_bytes: &[u8; 32],
    cancellation: Option<&CancellationToken>,
) -> Result<TimelockedFileVerification> {
    let (protected_stream, structural) =
        reconstruct_and_scan_protected_stream(path, parsed, cancellation)?;
    let digest = superblock_digest(&parsed.superblock)?;
    let decrypted = decrypt_protected_stream_to_writer_with_cancel(
        &mut Cursor::new(protected_stream),
        &mut sink(),
        key_bytes,
        &digest,
        parsed.superblock.aead_chunk_size_bytes,
        parsed.superblock.payload_plaintext_bytes,
        None,
        cancellation,
    )?;

    debug_assert_eq!(structural.chunk_count, decrypted.chunk_count);
    Ok(TimelockedFileVerification {
        chunk_count: decrypted.chunk_count,
        payload_plaintext_bytes: decrypted.plaintext_bytes,
    })
}

fn reconstruct_and_scan_protected_stream(
    path: &Path,
    parsed: &ParsedContainer,
    cancellation: Option<&CancellationToken>,
) -> Result<(Vec<u8>, ChunkDecryptionStats)> {
    ensure_not_cancelled(cancellation)?;

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
    let structural = scan_protected_stream_structural(
        &mut Cursor::new(&protected_stream),
        parsed.superblock.aead_chunk_size_bytes,
    )?;
    ensure_structural_plaintext_matches_superblock(parsed, structural.plaintext_bytes)?;
    Ok((protected_stream, structural))
}

fn ensure_structural_plaintext_matches_superblock(
    parsed: &ParsedContainer,
    structural_plaintext_bytes: u64,
) -> Result<()> {
    let expected = parsed.superblock.payload_plaintext_bytes;
    if structural_plaintext_bytes != expected {
        return Err(Error::Verification(format!(
            "payload plaintext size mismatch, expected {}, got {}",
            expected, structural_plaintext_bytes
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs::OpenOptions;
    use std::io::{Seek, SeekFrom, Write};

    use tempfile::tempdir;

    use super::{verify_timelocked_file_structural, TimelockedFileVerification};
    use crate::base::Error;
    use crate::domains::timelocked_file::{
        encode_end_superblock_copy, encode_start_superblock_copy, parse_container,
        test_support::SampleTimelockedFileBuilder,
    };

    #[test]
    fn structural_verification_reports_chunk_count_and_plaintext_size() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("payload.timelocked");
        SampleTimelockedFileBuilder::new(b"verify structurally")
            .write_to(&path)
            .expect("write artifact");
        let parsed = parse_container(&path).expect("parse artifact");

        let stats = verify_timelocked_file_structural(&path, &parsed).expect("verify");

        assert_eq!(
            stats,
            TimelockedFileVerification {
                chunk_count: 1,
                payload_plaintext_bytes: 19,
            }
        );
    }

    #[test]
    fn structural_verification_rejects_superblock_plaintext_size_mismatch() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("payload.timelocked");
        SampleTimelockedFileBuilder::new(b"verify structurally")
            .write_to(&path)
            .expect("write artifact");
        let parsed = parse_container(&path).expect("parse artifact");

        let mut modified = parsed.superblock.clone();
        modified.payload_plaintext_bytes += 1;
        replace_superblock_copies(&path, &parsed, &modified);

        let parsed = parse_container(&path).expect("parse modified artifact");
        let err = verify_timelocked_file_structural(&path, &parsed).expect_err("must fail");

        assert!(matches!(err, Error::Verification(_)));
        assert!(err.to_string().contains("payload plaintext size mismatch"));
    }

    fn replace_superblock_copies(
        path: &std::path::Path,
        parsed: &crate::domains::timelocked_file::ParsedContainer,
        superblock: &crate::domains::timelocked_file::SuperblockBody,
    ) {
        let start_copy = encode_start_superblock_copy(superblock).expect("encode start");
        let end_copy = encode_end_superblock_copy(superblock).expect("encode end");
        overwrite_bytes(path, parsed.start_superblock_offset, &start_copy);
        overwrite_bytes(path, parsed.end_superblock_offset, &end_copy);
    }

    fn overwrite_bytes(path: &std::path::Path, offset: u64, bytes: &[u8]) {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .expect("open file");
        file.seek(SeekFrom::Start(offset)).expect("seek");
        file.write_all(bytes).expect("write");
    }
}
