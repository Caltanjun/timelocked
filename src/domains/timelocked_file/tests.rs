//! End-to-end tests for the new Timelocked file format domain.

use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};

use tempfile::tempdir;

use crate::base::Error;
use crate::domains::timelocked_file::{
    choose_rs_shard_bytes_for_protected_stream_len, encode_end_superblock_copy,
    encode_start_superblock_copy, parse_container, predicted_protected_stream_len,
    test_support::SampleTimelockedFileBuilder, PayloadKind,
};

#[test]
fn artifact_kind_is_text_when_original_filename_is_absent() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("message.timelocked");

    let parsed = SampleTimelockedFileBuilder::new(b"hello")
        .write_and_parse(&path)
        .expect("parse");

    assert_eq!(parsed.payload_kind(), PayloadKind::Text);
}

#[test]
fn artifact_kind_is_file_when_original_filename_is_present() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("file.timelocked");

    let parsed = SampleTimelockedFileBuilder::new(b"hello")
        .original_filename("note.txt")
        .write_and_parse(&path)
        .expect("parse");

    assert_eq!(
        parsed.payload_kind(),
        PayloadKind::File {
            original_filename: "note.txt".to_string(),
        }
    );
}

#[test]
fn write_and_read_timelocked_artifact_round_trip_file_payload() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("file.timelocked");

    let parsed = SampleTimelockedFileBuilder::new(b"payload bytes")
        .original_filename("payload.bin")
        .target_seconds(Some(30))
        .write_and_parse(&path)
        .expect("parse");

    assert_eq!(
        parsed.header.original_filename.as_deref(),
        Some("payload.bin")
    );
    assert_eq!(parsed.superblock.payload_plaintext_bytes, 13);
    assert_eq!(parsed.superblock.target_seconds, Some(30));
}

#[test]
fn write_and_read_timelocked_artifact_round_trip_text_payload() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("message.timelocked");

    let parsed = SampleTimelockedFileBuilder::new("hello future".as_bytes())
        .write_and_parse(&path)
        .expect("parse");

    assert_eq!(parsed.payload_kind(), PayloadKind::Text);
    assert_eq!(parsed.superblock.payload_plaintext_bytes, 12);
}

#[test]
fn write_timelocked_artifact_uses_adaptive_rs_shard_bytes_for_small_payloads() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("small.timelocked");
    let plaintext = b"hello";

    let parsed = SampleTimelockedFileBuilder::new(plaintext)
        .write_and_parse(&path)
        .expect("parse");

    let protected_stream_len = predicted_protected_stream_len(
        plaintext.len() as u64,
        parsed.superblock.aead_chunk_size_bytes,
    )
    .expect("protected stream len");
    assert_eq!(parsed.superblock.protected_stream_len, protected_stream_len);
    assert_eq!(
        parsed.superblock.rs_shard_bytes,
        choose_rs_shard_bytes_for_protected_stream_len(protected_stream_len)
    );
}

#[test]
fn sample_timelocked_file_builder_preserves_explicit_rs_shard_override() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("override.timelocked");

    let parsed = SampleTimelockedFileBuilder::new(b"hello")
        .payload_region_params(4, 2, 512)
        .write_and_parse(&path)
        .expect("parse");

    assert_eq!(parsed.superblock.rs_data_shards, 4);
    assert_eq!(parsed.superblock.rs_parity_shards, 2);
    assert_eq!(parsed.superblock.rs_shard_bytes, 512);
}

#[test]
fn read_timelocked_artifact_recovers_from_valid_end_superblock_when_file_start_is_damaged() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("recover.timelocked");
    SampleTimelockedFileBuilder::new(b"recover me")
        .original_filename("recover.txt")
        .write_to(&path)
        .expect("write");

    overwrite_bytes(&path, 0, &[0_u8; 24]);

    let parsed = parse_container(&path).expect("parse from end copy");
    assert_eq!(
        parsed.header.original_filename.as_deref(),
        Some("recover.txt")
    );
    assert_eq!(parsed.superblock.payload_plaintext_bytes, 10);
}

#[test]
fn read_timelocked_artifact_rejects_payload_region_length_mismatch() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("mismatch.timelocked");
    SampleTimelockedFileBuilder::new(b"mismatch")
        .write_to(&path)
        .expect("write");

    let parsed = parse_container(&path).expect("parse original");
    let mut modified = parsed.superblock.clone();
    modified.payload_region_len += 1;
    replace_superblock_copies(&path, &parsed, &modified);

    let err = parse_container(&path).expect_err("must fail");
    assert!(matches!(err, Error::Verification(_)));
    assert!(err.to_string().contains("payload region length mismatch"));
}

#[test]
fn read_timelocked_artifact_rejects_two_different_valid_superblock_copies() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("contradict.timelocked");
    SampleTimelockedFileBuilder::new(b"contradict")
        .write_to(&path)
        .expect("write");

    let parsed = parse_container(&path).expect("parse original");
    let mut modified = parsed.superblock.clone();
    modified.created_at_unix_seconds += 1;
    let end_copy = encode_end_superblock_copy(&modified).expect("encode end");
    overwrite_bytes(&path, parsed.end_superblock_offset, &end_copy);

    let err = parse_container(&path).expect_err("must fail");
    assert!(matches!(err, Error::InvalidFormat(_)));
    assert!(err
        .to_string()
        .contains("contradictory valid superblock copies"));
}

#[test]
fn read_timelocked_artifact_preserves_created_at_original_filename_and_delay_metadata() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("metadata.timelocked");
    let parsed = SampleTimelockedFileBuilder::new(b"metadata")
        .original_filename("meta.txt")
        .hardware_profile("desktop-2026")
        .target_seconds(Some(45))
        .created_at_unix_seconds(1_700_000_123)
        .write_and_parse(&path)
        .expect("parse");

    assert_eq!(parsed.superblock.created_at_unix_seconds, 1_700_000_123);
    assert_eq!(parsed.header.original_filename.as_deref(), Some("meta.txt"));
    assert_eq!(parsed.superblock.target_seconds, Some(45));
    assert_eq!(parsed.superblock.hardware_profile, "desktop-2026");
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
