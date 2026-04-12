//! Orchestrates structural verification of a timelocked artifact.

use std::path::PathBuf;

use crate::base::progress_status::ProgressStatus;
use crate::base::{CancellationToken, Result};
use crate::domains::timelocked_file::{
    parse_container, verify_timelocked_file_structural_and_cancel,
};

#[derive(Debug, Clone)]
pub struct VerifyRequest {
    pub input: PathBuf,
}

#[derive(Debug, Clone)]
pub struct VerifyResponse {
    pub path: PathBuf,
    pub chunk_count: u64,
    pub payload_plaintext_bytes: u64,
}

pub fn execute(
    request: VerifyRequest,
    on_progress: Option<&mut dyn FnMut(ProgressStatus)>,
) -> Result<VerifyResponse> {
    execute_with_cancel(request, on_progress, None)
}

pub fn execute_with_cancel(
    request: VerifyRequest,
    _on_progress: Option<&mut dyn FnMut(ProgressStatus)>,
    cancellation: Option<&CancellationToken>,
) -> Result<VerifyResponse> {
    let parsed = parse_container(&request.input)?;
    let stats =
        verify_timelocked_file_structural_and_cancel(&request.input, &parsed, cancellation)?;

    Ok(VerifyResponse {
        path: request.input,
        chunk_count: stats.chunk_count,
        payload_plaintext_bytes: stats.payload_plaintext_bytes,
    })
}

#[cfg(test)]
mod tests {
    use std::fs::OpenOptions;
    use std::io::{Seek, SeekFrom, Write};

    use tempfile::tempdir;

    use crate::domains::timelocked_file::{
        encode_end_superblock_copy, encode_start_superblock_copy,
        test_support::SampleTimelockedFileBuilder,
    };

    use crate::usecases::unlock::{self, UnlockRequest};

    use super::{execute, VerifyRequest};

    #[test]
    fn verify_execute_returns_structural_stats() {
        let dir = tempdir().expect("tempdir");
        let output = dir.path().join("payload.txt.timelocked");
        SampleTimelockedFileBuilder::new(b"verify me")
            .original_filename("payload.txt")
            .write_to(&output)
            .expect("write sample artifact");

        let response = execute(VerifyRequest { input: output }, None).expect("verify");

        assert_eq!(response.payload_plaintext_bytes, 9);
        assert!(response.chunk_count >= 1);
    }

    #[test]
    fn verify_execute_ignores_tampered_wrapped_key_but_unlock_fails() {
        let dir = tempdir().expect("tempdir");
        let output = dir.path().join("payload.txt.timelocked");
        SampleTimelockedFileBuilder::new(b"verify me")
            .original_filename("payload.txt")
            .write_to(&output)
            .expect("write sample artifact");

        let parsed = crate::domains::timelocked_file::parse_container(&output).expect("parse");
        let mut modified = parsed.superblock.clone();
        modified.timelock_material.wrapped_key[0] ^= 0xFF;
        replace_superblock_copies(&output, &parsed, &modified);

        let response = execute(
            VerifyRequest {
                input: output.clone(),
            },
            None,
        )
        .expect("structural verify should still pass");

        assert_eq!(response.payload_plaintext_bytes, 9);

        let err = unlock::execute(
            UnlockRequest {
                input: output,
                out_dir: None,
                out: None,
            },
            None,
        )
        .expect_err("unlock must fail with wrong wrapped key");

        assert!(err.to_string().contains("payload authentication failed"));
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
