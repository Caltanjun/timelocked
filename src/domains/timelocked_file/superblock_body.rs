//! Authoritative superblock body encoding, decoding, and validation.
//! Owns the authenticated metadata and timelock material for v1 artifacts.

use std::io::{Cursor, Read};

use num_bigint::BigUint;
use num_traits::identities::Zero;

use crate::base::{Error, Result};

pub const BODY_VERSION_V1: u8 = 1;
pub const AEAD_CIPHER_ID_XCHACHA20POLY1305: u8 = 1;
pub const RS_ALGORITHM_ID_GF256_REED_SOLOMON: u8 = 1;
pub const TIMELOCK_ALGORITHM_ID_RSW_REPEATED_SQUARING_V1: u8 = 1;
pub const MAX_SUPERBLOCK_BODY_LEN_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelockPayloadMaterial {
    pub modulus_n: BigUint,
    pub base_a: BigUint,
    pub wrapped_key: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuperblockBody {
    pub body_version: u8,
    pub flags: u16,
    pub payload_plaintext_bytes: u64,
    pub protected_stream_len: u64,
    pub payload_region_len: u64,
    pub aead_chunk_size_bytes: u32,
    pub aead_cipher_id: u8,
    pub rs_algorithm_id: u8,
    pub rs_data_shards: u16,
    pub rs_parity_shards: u16,
    pub rs_shard_bytes: u32,
    pub timelock_algorithm_id: u8,
    pub iterations: u64,
    pub modulus_bits: u16,
    pub target_seconds: Option<u64>,
    pub created_at_unix_seconds: u64,
    pub original_filename: Option<String>,
    pub hardware_profile: String,
    pub timelock_material: TimelockPayloadMaterial,
}

impl SuperblockBody {
    pub fn validate(&self) -> Result<()> {
        if self.body_version != BODY_VERSION_V1 {
            return Err(Error::UnsupportedVersion(self.body_version));
        }
        if self.flags != 0 {
            return Err(Error::InvalidFormat(
                "unknown critical superblock flags".to_string(),
            ));
        }
        if self.aead_chunk_size_bytes == 0 {
            return Err(Error::InvalidFormat(
                "aead_chunk_size_bytes must be greater than 0".to_string(),
            ));
        }
        if self.rs_data_shards == 0 {
            return Err(Error::InvalidFormat(
                "rs_data_shards must be greater than 0".to_string(),
            ));
        }
        if self.rs_parity_shards == 0 {
            return Err(Error::InvalidFormat(
                "rs_parity_shards must be greater than 0".to_string(),
            ));
        }
        if self.rs_shard_bytes == 0 {
            return Err(Error::InvalidFormat(
                "rs_shard_bytes must be greater than 0".to_string(),
            ));
        }
        if self.aead_cipher_id != AEAD_CIPHER_ID_XCHACHA20POLY1305 {
            return Err(Error::InvalidFormat(
                "unknown AEAD algorithm id".to_string(),
            ));
        }
        if self.rs_algorithm_id != RS_ALGORITHM_ID_GF256_REED_SOLOMON {
            return Err(Error::InvalidFormat(
                "unknown Reed-Solomon algorithm id".to_string(),
            ));
        }
        if self.timelock_algorithm_id != TIMELOCK_ALGORITHM_ID_RSW_REPEATED_SQUARING_V1 {
            return Err(Error::InvalidFormat(
                "unknown timelock algorithm id".to_string(),
            ));
        }

        if self.timelock_material.modulus_n.is_zero() {
            return Err(Error::InvalidFormat(
                "modulus_n_len must be greater than 0".to_string(),
            ));
        }
        if self.timelock_material.base_a.is_zero() {
            return Err(Error::InvalidFormat(
                "base_a_len must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }
}

pub fn encode_superblock_body(body: &SuperblockBody) -> Result<Vec<u8>> {
    body.validate()?;

    let modulus_n_bytes = body.timelock_material.modulus_n.to_bytes_be();
    let base_a_bytes = body.timelock_material.base_a.to_bytes_be();
    let original_filename_bytes = body
        .original_filename
        .as_deref()
        .unwrap_or_default()
        .as_bytes();
    let hardware_profile_bytes = body.hardware_profile.as_bytes();

    let mut out = Vec::new();
    out.push(body.body_version);
    out.extend_from_slice(&body.flags.to_le_bytes());
    out.extend_from_slice(&body.payload_plaintext_bytes.to_le_bytes());
    out.extend_from_slice(&body.protected_stream_len.to_le_bytes());
    out.extend_from_slice(&body.payload_region_len.to_le_bytes());
    out.extend_from_slice(&body.aead_chunk_size_bytes.to_le_bytes());
    out.push(body.aead_cipher_id);
    out.push(body.rs_algorithm_id);
    out.extend_from_slice(&body.rs_data_shards.to_le_bytes());
    out.extend_from_slice(&body.rs_parity_shards.to_le_bytes());
    out.extend_from_slice(&body.rs_shard_bytes.to_le_bytes());
    out.push(body.timelock_algorithm_id);
    out.extend_from_slice(&body.iterations.to_le_bytes());
    out.extend_from_slice(&body.modulus_bits.to_le_bytes());
    match body.target_seconds {
        Some(target_seconds) => {
            out.push(1);
            out.extend_from_slice(&target_seconds.to_le_bytes());
        }
        None => {
            out.push(0);
            out.extend_from_slice(&0_u64.to_le_bytes());
        }
    }
    out.extend_from_slice(&body.created_at_unix_seconds.to_le_bytes());
    out.extend_from_slice(&(original_filename_bytes.len() as u16).to_le_bytes());
    out.extend_from_slice(original_filename_bytes);
    out.extend_from_slice(&(hardware_profile_bytes.len() as u16).to_le_bytes());
    out.extend_from_slice(hardware_profile_bytes);
    out.extend_from_slice(&(modulus_n_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&modulus_n_bytes);
    out.extend_from_slice(&(base_a_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&base_a_bytes);
    out.extend_from_slice(&body.timelock_material.wrapped_key);
    Ok(out)
}

pub fn decode_superblock_body(bytes: &[u8]) -> Result<SuperblockBody> {
    if bytes.len() > MAX_SUPERBLOCK_BODY_LEN_BYTES {
        return Err(Error::InvalidFormat(
            "superblock body exceeds maximum size".to_string(),
        ));
    }

    let mut cursor = Cursor::new(bytes);
    let body_version = read_u8(&mut cursor)?;
    let flags = read_u16(&mut cursor)?;
    let payload_plaintext_bytes = read_u64(&mut cursor)?;
    let protected_stream_len = read_u64(&mut cursor)?;
    let payload_region_len = read_u64(&mut cursor)?;
    let aead_chunk_size_bytes = read_u32(&mut cursor)?;
    let aead_cipher_id = read_u8(&mut cursor)?;
    let rs_algorithm_id = read_u8(&mut cursor)?;
    let rs_data_shards = read_u16(&mut cursor)?;
    let rs_parity_shards = read_u16(&mut cursor)?;
    let rs_shard_bytes = read_u32(&mut cursor)?;
    let timelock_algorithm_id = read_u8(&mut cursor)?;
    let iterations = read_u64(&mut cursor)?;
    let modulus_bits = read_u16(&mut cursor)?;
    let target_seconds_present = read_u8(&mut cursor)?;
    let raw_target_seconds = read_u64(&mut cursor)?;
    let target_seconds = match target_seconds_present {
        0 => None,
        1 => Some(raw_target_seconds),
        _ => {
            return Err(Error::InvalidFormat(
                "target_seconds_present must be 0 or 1".to_string(),
            ))
        }
    };
    let created_at_unix_seconds = read_u64(&mut cursor)?;
    let original_filename_len = read_u16(&mut cursor)? as usize;
    let original_filename = read_optional_utf8(&mut cursor, original_filename_len)?;
    let hardware_profile_len = read_u16(&mut cursor)? as usize;
    let hardware_profile = read_required_utf8(&mut cursor, hardware_profile_len)?;
    let modulus_n_len = read_u32(&mut cursor)? as usize;
    if modulus_n_len == 0 {
        return Err(Error::InvalidFormat(
            "modulus_n_len must be greater than 0".to_string(),
        ));
    }
    let modulus_n = BigUint::from_bytes_be(&read_exact_vec(&mut cursor, modulus_n_len)?);
    let base_a_len = read_u32(&mut cursor)? as usize;
    if base_a_len == 0 {
        return Err(Error::InvalidFormat(
            "base_a_len must be greater than 0".to_string(),
        ));
    }
    let base_a = BigUint::from_bytes_be(&read_exact_vec(&mut cursor, base_a_len)?);
    let mut wrapped_key = [0_u8; 32];
    cursor.read_exact(&mut wrapped_key)?;

    if cursor.position() != bytes.len() as u64 {
        return Err(Error::InvalidFormat(
            "unexpected trailing bytes in superblock body".to_string(),
        ));
    }

    let body = SuperblockBody {
        body_version,
        flags,
        payload_plaintext_bytes,
        protected_stream_len,
        payload_region_len,
        aead_chunk_size_bytes,
        aead_cipher_id,
        rs_algorithm_id,
        rs_data_shards,
        rs_parity_shards,
        rs_shard_bytes,
        timelock_algorithm_id,
        iterations,
        modulus_bits,
        target_seconds,
        created_at_unix_seconds,
        original_filename,
        hardware_profile,
        timelock_material: TimelockPayloadMaterial {
            modulus_n,
            base_a,
            wrapped_key,
        },
    };
    body.validate()?;
    Ok(body)
}

pub fn superblock_digest(body: &SuperblockBody) -> Result<[u8; 32]> {
    Ok(*blake3::hash(&encode_superblock_body(body)?).as_bytes())
}

fn read_u8(reader: &mut impl Read) -> Result<u8> {
    let mut bytes = [0_u8; 1];
    reader.read_exact(&mut bytes)?;
    Ok(bytes[0])
}

fn read_u16(reader: &mut impl Read) -> Result<u16> {
    let mut bytes = [0_u8; 2];
    reader.read_exact(&mut bytes)?;
    Ok(u16::from_le_bytes(bytes))
}

fn read_u32(reader: &mut impl Read) -> Result<u32> {
    let mut bytes = [0_u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(reader: &mut impl Read) -> Result<u64> {
    let mut bytes = [0_u8; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

fn read_exact_vec(reader: &mut impl Read, len: usize) -> Result<Vec<u8>> {
    let mut bytes = vec![0_u8; len];
    reader.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn read_optional_utf8(reader: &mut impl Read, len: usize) -> Result<Option<String>> {
    if len == 0 {
        return Ok(None);
    }
    let bytes = read_exact_vec(reader, len)?;
    let value = String::from_utf8(bytes)
        .map_err(|_| Error::InvalidFormat("original_filename_utf8 is invalid UTF-8".to_string()))?;
    Ok(Some(value))
}

fn read_required_utf8(reader: &mut impl Read, len: usize) -> Result<String> {
    let bytes = read_exact_vec(reader, len)?;
    String::from_utf8(bytes)
        .map_err(|_| Error::InvalidFormat("hardware_profile_utf8 is invalid UTF-8".to_string()))
}

#[cfg(test)]
mod tests {
    use num_bigint::BigUint;

    use crate::base::Error;

    use super::*;

    fn sample_body() -> SuperblockBody {
        SuperblockBody {
            body_version: BODY_VERSION_V1,
            flags: 0,
            payload_plaintext_bytes: 0,
            protected_stream_len: 57,
            payload_region_len: 60,
            aead_chunk_size_bytes: 8,
            aead_cipher_id: AEAD_CIPHER_ID_XCHACHA20POLY1305,
            rs_algorithm_id: RS_ALGORITHM_ID_GF256_REED_SOLOMON,
            rs_data_shards: 2,
            rs_parity_shards: 1,
            rs_shard_bytes: 8,
            timelock_algorithm_id: TIMELOCK_ALGORITHM_ID_RSW_REPEATED_SQUARING_V1,
            iterations: 10,
            modulus_bits: 256,
            target_seconds: Some(2),
            created_at_unix_seconds: 1_700_000_000,
            original_filename: None,
            hardware_profile: "desktop-2026".to_string(),
            timelock_material: TimelockPayloadMaterial {
                modulus_n: BigUint::from(3233_u32),
                base_a: BigUint::from(5_u32),
                wrapped_key: [9_u8; 32],
            },
        }
    }

    #[test]
    fn superblock_body_validation_accepts_minimal_valid_values() {
        sample_body().validate().expect("valid");
    }

    #[test]
    fn superblock_body_validation_rejects_zero_aead_chunk_size() {
        let mut body = sample_body();
        body.aead_chunk_size_bytes = 0;
        let err = body.validate().expect_err("must fail");
        assert!(matches!(err, Error::InvalidFormat(_)));
    }

    #[test]
    fn superblock_body_validation_rejects_zero_rs_data_shards() {
        let mut body = sample_body();
        body.rs_data_shards = 0;
        let err = body.validate().expect_err("must fail");
        assert!(matches!(err, Error::InvalidFormat(_)));
    }

    #[test]
    fn superblock_body_validation_rejects_zero_rs_parity_shards() {
        let mut body = sample_body();
        body.rs_parity_shards = 0;
        let err = body.validate().expect_err("must fail");
        assert!(matches!(err, Error::InvalidFormat(_)));
    }

    #[test]
    fn superblock_body_validation_rejects_zero_rs_shard_bytes() {
        let mut body = sample_body();
        body.rs_shard_bytes = 0;
        let err = body.validate().expect_err("must fail");
        assert!(matches!(err, Error::InvalidFormat(_)));
    }

    #[test]
    fn superblock_body_validation_rejects_zero_modulus_n_len() {
        let mut body = sample_body();
        body.timelock_material.modulus_n = BigUint::default();
        let err = body.validate().expect_err("must fail");
        assert!(matches!(err, Error::InvalidFormat(_)));
    }

    #[test]
    fn superblock_body_validation_rejects_zero_base_a_len() {
        let mut body = sample_body();
        body.timelock_material.base_a = BigUint::default();
        let err = body.validate().expect_err("must fail");
        assert!(matches!(err, Error::InvalidFormat(_)));
    }

    #[test]
    fn superblock_body_round_trips_exactly() {
        let body = sample_body();
        let encoded = encode_superblock_body(&body).expect("encode");
        let decoded = decode_superblock_body(&encoded).expect("decode");
        assert_eq!(decoded, body);
    }

    #[test]
    fn decode_superblock_body_rejects_truncated_input() {
        let body = sample_body();
        let mut encoded = encode_superblock_body(&body).expect("encode");
        encoded.pop();

        let err = decode_superblock_body(&encoded).expect_err("must fail");
        assert!(matches!(err, Error::Io(_)));
    }

    #[test]
    fn decode_superblock_body_rejects_unknown_critical_flags() {
        let mut bytes = encode_superblock_body(&sample_body()).expect("encode valid");
        bytes[1] = 1;
        let err = decode_superblock_body(&bytes).expect_err("must fail");
        assert!(matches!(err, Error::InvalidFormat(_)));
        assert!(err
            .to_string()
            .contains("unknown critical superblock flags"));
    }

    #[test]
    fn created_at_unix_seconds_round_trips_exactly() {
        let body = sample_body();
        let encoded = encode_superblock_body(&body).expect("encode");
        let decoded = decode_superblock_body(&encoded).expect("decode");
        assert_eq!(
            decoded.created_at_unix_seconds,
            body.created_at_unix_seconds
        );
    }
}
