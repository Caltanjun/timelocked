//! Domain definitions and business rules for the new `.timelocked` binary format.
//! Owns the authoritative superblock, payload-region recovery, and artifact parsing/writing.

mod artifact;
mod chunk_frame;
mod header;
mod lock_output_path;
mod notice;
mod output_path;
mod payload_region;
mod protected_stream;
mod reader;
mod recovery;
mod reed_solomon;
mod shard_record;
mod superblock;
mod superblock_body;
#[cfg(test)]
pub(crate) mod test_support;
mod verification;
mod writer;

#[cfg(test)]
mod tests;

pub use artifact::{ParsedContainer, PayloadKind, StoredTimelockedArtifact};
pub use chunk_frame::{build_chunk_aad, read_chunk_frame, write_chunk_frame, ChunkFrameHeader};
pub use header::{ChunkingParams, CipherParams, TimelockParams, TimelockedHeader};
pub use lock_output_path::{default_timelocked_output_path, ensure_timelocked_extension};
pub use output_path::resolve_available_output_path;
pub use payload_region::{encode_payload_region, reconstruct_payload_region, PayloadRegionLayout};
pub use protected_stream::{
    decrypt_protected_stream_to_writer, decrypt_protected_stream_to_writer_with_cancel,
    encrypt_protected_stream, encrypt_protected_stream_with_cancel,
    scan_protected_stream_structural, ChunkDecryptionStats, ChunkEncryptionStats,
};
pub use reader::{parse_container, read_timelocked_artifact};
pub use recovery::{
    recover_payload_to_writer_with_cancel, recover_protected_stream_to_writer_with_cancel,
    RecoverFileKeyRequest,
};
pub use shard_record::{decode_shard_record, encode_shard_record};
pub use superblock::{
    choose_superblock_copy, decode_end_superblock_copy, decode_start_superblock_copy,
    encode_end_superblock_copy, encode_start_superblock_copy, ValidatedSuperblockCopy,
};
pub use superblock_body::{
    decode_superblock_body, encode_superblock_body, superblock_digest, SuperblockBody,
    TimelockPayloadMaterial, AEAD_CIPHER_ID_XCHACHA20POLY1305, BODY_VERSION_V1,
    RS_ALGORITHM_ID_GF256_REED_SOLOMON, TIMELOCK_ALGORITHM_ID_RSW_REPEATED_SQUARING_V1,
};
pub use verification::{
    verify_timelocked_file_structural, verify_timelocked_file_structural_and_cancel,
    verify_timelocked_file_with_key, verify_timelocked_file_with_key_and_cancel,
    TimelockedFileVerification,
};
pub use writer::{
    choose_rs_shard_bytes_for_protected_stream_len, predicted_protected_stream_len,
    write_timelocked_artifact, LockArtifactRequest, PayloadRegionEncodingParams,
    DEFAULT_LOCK_CHUNK_SIZE_BYTES, DEFAULT_RS_DATA_SHARDS, DEFAULT_RS_PARITY_SHARDS,
    DEFAULT_RS_SHARD_BYTES,
};
