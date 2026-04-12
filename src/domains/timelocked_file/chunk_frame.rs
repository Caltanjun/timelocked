//! Low-level reading and writing mechanics for individual encrypted chunks,
//! ensuring sequence integrity and boundary conditions are respected.

use std::io::{Read, Write};

use crate::base::{Error, Result};

const AEAD_TAG_BYTES: u32 = 16;

#[derive(Debug, Clone)]
pub struct ChunkFrameHeader {
    pub index: u64,
    pub is_last: bool,
    pub plaintext_len: u32,
    pub ciphertext_len: u32,
    pub nonce: [u8; 24],
}

pub fn write_chunk_frame(
    writer: &mut impl Write,
    frame: &ChunkFrameHeader,
    ciphertext: &[u8],
) -> Result<()> {
    if frame.ciphertext_len as usize != ciphertext.len() {
        return Err(Error::InvalidArgument(
            "ciphertext_len does not match ciphertext bytes".to_string(),
        ));
    }

    writer.write_all(&frame.index.to_le_bytes())?;
    writer.write_all(&[u8::from(frame.is_last)])?;
    writer.write_all(&frame.plaintext_len.to_le_bytes())?;
    writer.write_all(&frame.ciphertext_len.to_le_bytes())?;
    writer.write_all(&frame.nonce)?;
    writer.write_all(ciphertext)?;
    Ok(())
}

pub fn read_chunk_frame(reader: &mut impl Read) -> Result<Option<(ChunkFrameHeader, Vec<u8>)>> {
    let maybe_index = read_exact_or_eof::<8>(reader)?;
    let Some(index_bytes) = maybe_index else {
        return Ok(None);
    };

    let index = u64::from_le_bytes(index_bytes);
    let is_last = read_u8(reader)? != 0;
    let plaintext_len = read_u32(reader)?;
    let ciphertext_len = read_u32(reader)?;

    if ciphertext_len < AEAD_TAG_BYTES {
        return Err(Error::InvalidFormat(
            "ciphertext length must include AEAD tag".to_string(),
        ));
    }

    let mut nonce = [0_u8; 24];
    reader.read_exact(&mut nonce)?;
    let mut ciphertext = vec![0_u8; ciphertext_len as usize];
    reader.read_exact(&mut ciphertext)?;

    Ok(Some((
        ChunkFrameHeader {
            index,
            is_last,
            plaintext_len,
            ciphertext_len,
            nonce,
        },
        ciphertext,
    )))
}

pub fn validate_chunk_frame(
    frame: &ChunkFrameHeader,
    ciphertext_len: usize,
    chunk_size: u32,
    expected_index: u64,
) -> Result<()> {
    if frame.index != expected_index {
        return Err(Error::InvalidFormat(format!(
            "non-contiguous chunk index at {}, got {}",
            expected_index, frame.index
        )));
    }
    if frame.plaintext_len > chunk_size {
        return Err(Error::InvalidFormat(format!(
            "chunk {} plaintext_len {} exceeds configured chunk size {}",
            frame.index, frame.plaintext_len, chunk_size
        )));
    }
    if frame.ciphertext_len != frame.plaintext_len + AEAD_TAG_BYTES {
        return Err(Error::InvalidFormat(format!(
            "chunk {} ciphertext_len mismatch",
            frame.index
        )));
    }
    if ciphertext_len as u32 != frame.ciphertext_len {
        return Err(Error::InvalidFormat(format!(
            "chunk {} ciphertext bytes truncated",
            frame.index
        )));
    }

    Ok(())
}

pub fn build_chunk_aad(
    superblock_digest: &[u8; 32],
    index: u64,
    plaintext_len: u32,
    is_last: bool,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + 32 + 8 + 4 + 1);
    out.extend_from_slice(b"TLCK-CHUNK-AD-v1");
    out.extend_from_slice(superblock_digest);
    out.extend_from_slice(&index.to_le_bytes());
    out.extend_from_slice(&plaintext_len.to_le_bytes());
    out.push(u8::from(is_last));
    out
}

fn read_u8(reader: &mut impl Read) -> Result<u8> {
    let mut byte = [0_u8; 1];
    reader.read_exact(&mut byte)?;
    Ok(byte[0])
}

fn read_u32(reader: &mut impl Read) -> Result<u32> {
    let mut bytes = [0_u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_exact_or_eof<const N: usize>(reader: &mut impl Read) -> Result<Option<[u8; N]>> {
    let mut out = [0_u8; N];
    let mut filled = 0_usize;
    while filled < N {
        let read = reader.read(&mut out[filled..])?;
        if read == 0 {
            if filled == 0 {
                return Ok(None);
            }
            return Err(Error::InvalidFormat(
                "unexpected eof while reading chunk frame".to_string(),
            ));
        }
        filled += read;
    }
    Ok(Some(out))
}
