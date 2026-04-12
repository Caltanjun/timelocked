//! Parsed and validated view of a stored `.timelocked` artifact.

use super::{SuperblockBody, TimelockedHeader};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PayloadKind {
    Text,
    File { original_filename: String },
}

#[derive(Debug, Clone)]
pub struct StoredTimelockedArtifact {
    pub notice: String,
    pub superblock: SuperblockBody,
    pub superblock_bytes: Vec<u8>,
    pub header: TimelockedHeader,
    pub start_superblock_offset: u64,
    pub payload_region_offset: u64,
    pub end_superblock_offset: u64,
    pub file_len: u64,
}

pub type ParsedContainer = StoredTimelockedArtifact;

impl StoredTimelockedArtifact {
    pub fn payload_kind(&self) -> PayloadKind {
        match self.superblock.original_filename.as_deref() {
            Some(original_filename) => PayloadKind::File {
                original_filename: original_filename.to_string(),
            },
            None => PayloadKind::Text,
        }
    }

    pub fn payload_region_len(&self) -> u64 {
        self.superblock.payload_region_len
    }
}
