//! Reader and recovery-oriented parser for `.timelocked` artifacts.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::base::{Error, Result};

use super::artifact::StoredTimelockedArtifact;
use super::notice::{decode_notice, find_start_superblock_offset, DEFAULT_NOTICE_TEXT};
use super::superblock::{
    choose_superblock_copy, decode_end_superblock_copy, decode_start_superblock_copy,
};
use super::TimelockedHeader;

pub fn parse_container(path: &Path) -> Result<StoredTimelockedArtifact> {
    read_timelocked_artifact(path)
}

pub fn read_timelocked_artifact(path: &Path) -> Result<StoredTimelockedArtifact> {
    let mut bytes = Vec::new();
    File::open(path)?.read_to_end(&mut bytes)?;
    read_timelocked_artifact_bytes(&bytes)
}

fn read_timelocked_artifact_bytes(bytes: &[u8]) -> Result<StoredTimelockedArtifact> {
    if bytes.len() < 6 {
        return Err(Error::InvalidFormat("file is too small".to_string()));
    }

    let start_copy = find_start_superblock_offset(bytes).ok().and_then(|offset| {
        decode_start_superblock_copy(&bytes[offset..])
            .ok()
            .map(|copy| (offset as u64, copy))
    });

    let end_copy = decode_end_copy_from_file(bytes)
        .ok()
        .map(|(offset, copy)| (offset as u64, copy));

    let chosen = choose_superblock_copy(
        start_copy.as_ref().map(|(_, copy)| copy.clone()),
        end_copy.as_ref().map(|(_, copy)| copy.clone()),
    )?;

    let start_superblock_offset = match start_copy.as_ref() {
        Some((offset, _)) => *offset,
        None => {
            let end = end_copy
                .as_ref()
                .expect("chosen start or end copy must exist");
            end.0
                .checked_sub(chosen.body.payload_region_len)
                .and_then(|payload_region_offset| {
                    payload_region_offset.checked_sub(chosen.total_len)
                })
                .ok_or_else(|| {
                    Error::InvalidFormat(
                        "invalid offsets while recovering from end superblock".to_string(),
                    )
                })?
        }
    };

    let payload_region_offset = match start_copy.as_ref() {
        Some((offset, copy)) => offset
            .checked_add(copy.total_len)
            .ok_or_else(|| Error::InvalidFormat("payload region offset overflow".to_string()))?,
        None => {
            let end = end_copy
                .as_ref()
                .expect("chosen start or end copy must exist");
            end.0
                .checked_sub(chosen.body.payload_region_len)
                .ok_or_else(|| {
                    Error::InvalidFormat(
                        "invalid payload region offset from end superblock".to_string(),
                    )
                })?
        }
    };

    let end_superblock_offset = match end_copy.as_ref() {
        Some((offset, _)) => *offset,
        None => payload_region_offset
            .checked_add(chosen.body.payload_region_len)
            .ok_or_else(|| Error::InvalidFormat("end superblock offset overflow".to_string()))?,
    };

    let file_len = bytes.len() as u64;
    let payload_region_end = payload_region_offset
        .checked_add(chosen.body.payload_region_len)
        .ok_or_else(|| Error::InvalidFormat("payload region length overflow".to_string()))?;

    if payload_region_end > file_len {
        return Err(Error::Verification(
            "payload region length mismatch".to_string(),
        ));
    }
    if end_superblock_offset < payload_region_end {
        return Err(Error::Verification(
            "payload region length mismatch".to_string(),
        ));
    }

    let notice = decode_notice(bytes)
        .ok()
        .map(|(notice, _)| notice)
        .unwrap_or_else(|| DEFAULT_NOTICE_TEXT.to_string());
    let header = TimelockedHeader::from_superblock(&chosen.body, &notice);

    Ok(StoredTimelockedArtifact {
        notice,
        superblock: chosen.body,
        superblock_bytes: chosen.body_bytes,
        header,
        start_superblock_offset,
        payload_region_offset,
        end_superblock_offset,
        file_len,
    })
}

fn decode_end_copy_from_file(bytes: &[u8]) -> Result<(usize, super::ValidatedSuperblockCopy)> {
    if bytes.len() < 12 {
        return Err(Error::InvalidFormat("file is too small".to_string()));
    }
    if &bytes[bytes.len() - 4..] != b"TLEB" {
        return Err(Error::InvalidFormat(
            "invalid end superblock magic".to_string(),
        ));
    }
    let len_offset = bytes.len() - 8;
    let crc_offset = bytes.len() - 12;
    let body_len = u32::from_le_bytes(
        bytes[len_offset..bytes.len() - 4]
            .try_into()
            .expect("slice"),
    ) as usize;
    let total_len = body_len + 12;
    if bytes.len() < total_len {
        return Err(Error::InvalidFormat("truncated end superblock".to_string()));
    }
    let start = bytes.len() - total_len;
    let _ = crc_offset;
    Ok((start, decode_end_superblock_copy(&bytes[start..])?))
}

pub(crate) fn read_payload_region_bytes(
    path: &Path,
    artifact: &StoredTimelockedArtifact,
) -> Result<Vec<u8>> {
    let bytes = std::fs::read(path)?;
    let start = artifact.payload_region_offset as usize;
    let end = start
        .checked_add(artifact.superblock.payload_region_len as usize)
        .ok_or_else(|| Error::InvalidFormat("payload region length overflow".to_string()))?;
    if end > bytes.len() {
        return Err(Error::Verification(
            "payload region length mismatch".to_string(),
        ));
    }
    Ok(bytes[start..end].to_vec())
}
