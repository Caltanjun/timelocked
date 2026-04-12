//! Defines the fixed-size binary prefix for all `.timelocked` files.
//! Contains magic bytes, format version, and exact lengths of subsequent sections.

use std::io::{Read, Write};

use crate::base::{Error, Result};

pub const MAGIC: [u8; 4] = *b"TLCK";
pub const FORMAT_VERSION: u8 = 1;

const PREFIX_LEN: usize = 4 + 1 + 2 + 4 + 8;

#[derive(Debug, Clone, Copy)]
pub struct FilePrefix {
    pub version: u8,
    pub flags: u16,
    pub header_len: u32,
    pub payload_len: u64,
}

pub fn write_file_prefix(
    writer: &mut impl Write,
    version: u8,
    flags: u16,
    header_len: u32,
    payload_len: u64,
) -> Result<()> {
    writer.write_all(&MAGIC)?;
    writer.write_all(&[version])?;
    writer.write_all(&flags.to_le_bytes())?;
    writer.write_all(&header_len.to_le_bytes())?;
    writer.write_all(&payload_len.to_le_bytes())?;
    Ok(())
}

pub fn read_file_prefix(reader: &mut impl Read) -> Result<FilePrefix> {
    let mut prefix = [0_u8; PREFIX_LEN];
    reader.read_exact(&mut prefix)?;

    if prefix[0..4] != MAGIC {
        return Err(Error::InvalidFormat("invalid magic bytes".to_string()));
    }

    let version = prefix[4];
    if version != FORMAT_VERSION {
        return Err(Error::UnsupportedVersion(version));
    }

    let flags = u16::from_le_bytes([prefix[5], prefix[6]]);
    let header_len = u32::from_le_bytes([prefix[7], prefix[8], prefix[9], prefix[10]]);
    let payload_len = u64::from_le_bytes([
        prefix[11], prefix[12], prefix[13], prefix[14], prefix[15], prefix[16], prefix[17],
        prefix[18],
    ]);

    Ok(FilePrefix {
        version,
        flags,
        header_len,
        payload_len,
    })
}
