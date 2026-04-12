//! Superblock wrapper framing, CRC validation, and copy selection rules.

use crate::base::{Error, Result};

use super::superblock_body::{
    decode_superblock_body, encode_superblock_body, SuperblockBody, MAX_SUPERBLOCK_BODY_LEN_BYTES,
};

pub const START_SUPERBLOCK_MAGIC: &[u8; 4] = b"TLSB";
pub const END_SUPERBLOCK_MAGIC: &[u8; 4] = b"TLEB";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedSuperblockCopy {
    pub body: SuperblockBody,
    pub body_bytes: Vec<u8>,
    pub total_len: u64,
}

pub fn encode_start_superblock_copy(body: &SuperblockBody) -> Result<Vec<u8>> {
    let body_bytes = encode_superblock_body(body)?;
    let crc = crc32c::crc32c(&body_bytes);
    let mut out = Vec::with_capacity(body_bytes.len() + 12);
    out.extend_from_slice(START_SUPERBLOCK_MAGIC);
    out.extend_from_slice(&(body_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&body_bytes);
    Ok(out)
}

pub fn encode_end_superblock_copy(body: &SuperblockBody) -> Result<Vec<u8>> {
    let body_bytes = encode_superblock_body(body)?;
    let crc = crc32c::crc32c(&body_bytes);
    let mut out = Vec::with_capacity(body_bytes.len() + 12);
    out.extend_from_slice(&body_bytes);
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&(body_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(END_SUPERBLOCK_MAGIC);
    Ok(out)
}

pub fn decode_start_superblock_copy(bytes: &[u8]) -> Result<ValidatedSuperblockCopy> {
    if bytes.len() < 12 || &bytes[..4] != START_SUPERBLOCK_MAGIC {
        return Err(Error::InvalidFormat(
            "invalid start superblock magic".to_string(),
        ));
    }
    let len = u32::from_le_bytes(bytes[4..8].try_into().expect("slice")) as usize;
    if len > MAX_SUPERBLOCK_BODY_LEN_BYTES {
        return Err(Error::InvalidFormat(
            "unreasonable start superblock length".to_string(),
        ));
    }
    let total_len = 12 + len;
    if bytes.len() < total_len {
        return Err(Error::InvalidFormat(
            "truncated start superblock".to_string(),
        ));
    }
    let crc = u32::from_le_bytes(bytes[8..12].try_into().expect("slice"));
    let body_bytes = bytes[12..total_len].to_vec();
    validate_crc(&body_bytes, crc)?;
    let body = decode_superblock_body(&body_bytes)?;
    Ok(ValidatedSuperblockCopy {
        body,
        body_bytes,
        total_len: total_len as u64,
    })
}

pub fn decode_end_superblock_copy(bytes: &[u8]) -> Result<ValidatedSuperblockCopy> {
    if bytes.len() < 12 || &bytes[bytes.len() - 4..] != END_SUPERBLOCK_MAGIC {
        return Err(Error::InvalidFormat(
            "invalid end superblock magic".to_string(),
        ));
    }

    let len_offset = bytes.len() - 8;
    let crc_offset = bytes.len() - 12;
    let len = u32::from_le_bytes(
        bytes[len_offset..bytes.len() - 4]
            .try_into()
            .expect("slice"),
    ) as usize;
    if len > MAX_SUPERBLOCK_BODY_LEN_BYTES {
        return Err(Error::InvalidFormat(
            "unreasonable end superblock length".to_string(),
        ));
    }
    if bytes.len() != len + 12 {
        return Err(Error::InvalidFormat(
            "end superblock length mismatch".to_string(),
        ));
    }
    let body_end = crc_offset;
    let body_bytes = bytes[..body_end].to_vec();
    let crc = u32::from_le_bytes(bytes[crc_offset..len_offset].try_into().expect("slice"));
    validate_crc(&body_bytes, crc)?;
    let body = decode_superblock_body(&body_bytes)?;
    Ok(ValidatedSuperblockCopy {
        body,
        body_bytes,
        total_len: bytes.len() as u64,
    })
}

pub fn choose_superblock_copy(
    start_copy: Option<ValidatedSuperblockCopy>,
    end_copy: Option<ValidatedSuperblockCopy>,
) -> Result<ValidatedSuperblockCopy> {
    match (start_copy, end_copy) {
        (Some(start), Some(end)) => {
            if start.body_bytes == end.body_bytes {
                Ok(start)
            } else {
                Err(Error::InvalidFormat(
                    "contradictory valid superblock copies".to_string(),
                ))
            }
        }
        (Some(start), None) => Ok(start),
        (None, Some(end)) => Ok(end),
        (None, None) => Err(Error::InvalidFormat(
            "both superblock copies are unusable".to_string(),
        )),
    }
}

fn validate_crc(body_bytes: &[u8], expected_crc: u32) -> Result<()> {
    let actual_crc = crc32c::crc32c(body_bytes);
    if actual_crc != expected_crc {
        return Err(Error::InvalidFormat("superblock CRC mismatch".to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use num_bigint::BigUint;

    use crate::base::Error;

    use super::*;
    use crate::domains::timelocked_file::notice::{encode_notice, find_start_superblock_offset};
    use crate::domains::timelocked_file::{
        SuperblockBody, TimelockPayloadMaterial, AEAD_CIPHER_ID_XCHACHA20POLY1305, BODY_VERSION_V1,
        RS_ALGORITHM_ID_GF256_REED_SOLOMON, TIMELOCK_ALGORITHM_ID_RSW_REPEATED_SQUARING_V1,
    };

    fn sample_body() -> SuperblockBody {
        SuperblockBody {
            body_version: BODY_VERSION_V1,
            flags: 0,
            payload_plaintext_bytes: 12,
            protected_stream_len: 69,
            payload_region_len: 120,
            aead_chunk_size_bytes: 8,
            aead_cipher_id: AEAD_CIPHER_ID_XCHACHA20POLY1305,
            rs_algorithm_id: RS_ALGORITHM_ID_GF256_REED_SOLOMON,
            rs_data_shards: 2,
            rs_parity_shards: 1,
            rs_shard_bytes: 16,
            timelock_algorithm_id: TIMELOCK_ALGORITHM_ID_RSW_REPEATED_SQUARING_V1,
            iterations: 42,
            modulus_bits: 256,
            target_seconds: Some(5),
            created_at_unix_seconds: 1_700_000_000,
            original_filename: Some("note.txt".to_string()),
            hardware_profile: "desktop-2026".to_string(),
            timelock_material: TimelockPayloadMaterial {
                modulus_n: BigUint::from(3233_u32),
                base_a: BigUint::from(5_u32),
                wrapped_key: [7_u8; 32],
            },
        }
    }

    #[test]
    fn start_superblock_wrapper_round_trips_exactly() {
        let body = sample_body();
        let encoded = encode_start_superblock_copy(&body).expect("encode");
        let decoded = decode_start_superblock_copy(&encoded).expect("decode");
        assert_eq!(decoded.body, body);
    }

    #[test]
    fn end_superblock_wrapper_round_trips_exactly() {
        let body = sample_body();
        let encoded = encode_end_superblock_copy(&body).expect("encode");
        let decoded = decode_end_superblock_copy(&encoded).expect("decode");
        assert_eq!(decoded.body, body);
    }

    #[test]
    fn invalid_superblock_crc_rejects_copy() {
        let body = sample_body();
        let mut encoded = encode_start_superblock_copy(&body).expect("encode");
        encoded[12] ^= 0xFF;

        let err = decode_start_superblock_copy(&encoded).expect_err("must fail");
        assert!(matches!(err, Error::InvalidFormat(_)));
        assert!(err.to_string().contains("CRC mismatch"));
    }

    #[test]
    fn choose_superblock_copy_accepts_identical_valid_copies() {
        let body = sample_body();
        let start = decode_start_superblock_copy(&encode_start_superblock_copy(&body).unwrap())
            .expect("start");
        let end =
            decode_end_superblock_copy(&encode_end_superblock_copy(&body).unwrap()).expect("end");
        let chosen = choose_superblock_copy(Some(start), Some(end)).expect("choose");
        assert_eq!(chosen.body, body);
    }

    #[test]
    fn choose_superblock_copy_accepts_only_valid_start_copy() {
        let body = sample_body();
        let start = decode_start_superblock_copy(&encode_start_superblock_copy(&body).unwrap())
            .expect("start");
        let chosen = choose_superblock_copy(Some(start), None).expect("choose");
        assert_eq!(chosen.body, body);
    }

    #[test]
    fn choose_superblock_copy_accepts_only_valid_end_copy() {
        let body = sample_body();
        let end =
            decode_end_superblock_copy(&encode_end_superblock_copy(&body).unwrap()).expect("end");
        let chosen = choose_superblock_copy(None, Some(end)).expect("choose");
        assert_eq!(chosen.body, body);
    }

    #[test]
    fn choose_superblock_copy_rejects_two_different_valid_copies() {
        let start =
            decode_start_superblock_copy(&encode_start_superblock_copy(&sample_body()).unwrap())
                .expect("start");
        let mut other = sample_body();
        other.created_at_unix_seconds += 1;
        let end =
            decode_end_superblock_copy(&encode_end_superblock_copy(&other).unwrap()).expect("end");

        let err = choose_superblock_copy(Some(start), Some(end)).expect_err("must fail");
        assert!(matches!(err, Error::InvalidFormat(_)));
        assert!(err
            .to_string()
            .contains("contradictory valid superblock copies"));
    }

    #[test]
    fn bounded_notice_scan_finds_valid_start_superblock_after_notice_damage() {
        let body = sample_body();
        let mut artifact = encode_notice("valid notice").expect("notice");
        artifact.extend_from_slice(&encode_start_superblock_copy(&body).expect("start"));
        artifact[4] = 0xFF;
        artifact[5] = 0xFF;

        let offset = find_start_superblock_offset(&artifact).expect("find");
        assert_eq!(offset, 6 + "valid notice".len());
    }
}
