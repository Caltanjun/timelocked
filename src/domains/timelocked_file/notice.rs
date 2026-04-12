//! Human-readable notice encoding plus bounded fallback scanning for the start superblock.

use crate::base::{Error, Result};

use super::superblock::{decode_start_superblock_copy, START_SUPERBLOCK_MAGIC};

pub const FILE_MAGIC: &[u8; 4] = b"TLCK";
pub const MAX_NOTICE_BYTES: usize = 4096;
pub const NOTICE_SCAN_WINDOW_BYTES: usize = 8192;
pub const DEFAULT_NOTICE_TEXT: &str =
    "This is a Timelocked file. Use the Timelocked app (https://timelocked.app) to inspect or unlock it.";

pub fn encode_notice(notice: &str) -> Result<Vec<u8>> {
    let notice_bytes = notice.as_bytes();
    if notice_bytes.len() > MAX_NOTICE_BYTES {
        return Err(Error::InvalidArgument(
            "notice exceeds supported maximum length".to_string(),
        ));
    }
    let mut out = Vec::with_capacity(6 + notice_bytes.len());
    out.extend_from_slice(FILE_MAGIC);
    out.extend_from_slice(&(notice_bytes.len() as u16).to_le_bytes());
    out.extend_from_slice(notice_bytes);
    Ok(out)
}

pub fn decode_notice(bytes: &[u8]) -> Result<(String, usize)> {
    if bytes.len() < 6 || &bytes[..4] != FILE_MAGIC {
        return Err(Error::InvalidFormat("invalid file magic".to_string()));
    }
    let notice_len = u16::from_le_bytes(bytes[4..6].try_into().expect("slice")) as usize;
    if notice_len > MAX_NOTICE_BYTES {
        return Err(Error::InvalidFormat("invalid notice length".to_string()));
    }
    let end = 6 + notice_len;
    if bytes.len() < end {
        return Err(Error::InvalidFormat("truncated notice".to_string()));
    }
    let notice = String::from_utf8(bytes[6..end].to_vec())
        .map_err(|_| Error::InvalidFormat("notice is not valid UTF-8".to_string()))?;
    Ok((notice, end))
}

pub fn find_start_superblock_offset(bytes: &[u8]) -> Result<usize> {
    if bytes.len() < 6 || &bytes[..4] != FILE_MAGIC {
        return Err(Error::InvalidFormat("invalid file magic".to_string()));
    }

    if let Ok((_, notice_end)) = decode_notice(bytes) {
        if bytes.len() >= notice_end + 4
            && &bytes[notice_end..notice_end + 4] == START_SUPERBLOCK_MAGIC
            && decode_start_superblock_copy(&bytes[notice_end..]).is_ok()
        {
            return Ok(notice_end);
        }
    }

    let scan_end = bytes.len().min(6 + NOTICE_SCAN_WINDOW_BYTES);
    for offset in 6..scan_end.saturating_sub(3) {
        if &bytes[offset..offset + 4] == START_SUPERBLOCK_MAGIC
            && decode_start_superblock_copy(&bytes[offset..]).is_ok()
        {
            return Ok(offset);
        }
    }

    Err(Error::InvalidFormat(
        "unable to locate a valid start superblock".to_string(),
    ))
}
