//! Shard-record framing with CRC32C for cheap corruption classification.

use crate::base::{Error, Result};

pub fn encode_shard_record(shard_bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + shard_bytes.len());
    out.extend_from_slice(&crc32c::crc32c(shard_bytes).to_le_bytes());
    out.extend_from_slice(shard_bytes);
    out
}

pub fn decode_shard_record(record_bytes: &[u8], shard_len: usize) -> Result<Option<Vec<u8>>> {
    if record_bytes.len() != shard_len + 4 {
        return Err(Error::InvalidFormat(
            "invalid shard record length".to_string(),
        ));
    }
    let expected_crc = u32::from_le_bytes(record_bytes[..4].try_into().expect("slice"));
    let shard_bytes = &record_bytes[4..];
    let actual_crc = crc32c::crc32c(shard_bytes);
    if actual_crc != expected_crc {
        return Ok(None);
    }
    Ok(Some(shard_bytes.to_vec()))
}

#[cfg(test)]
mod tests {
    use crate::base::Error;

    use super::{decode_shard_record, encode_shard_record};

    #[test]
    fn shard_record_round_trips_exactly() {
        let shard = b"hello shard";
        let record = encode_shard_record(shard);
        let decoded = decode_shard_record(&record, shard.len()).expect("decode");
        assert_eq!(decoded, Some(shard.to_vec()));
    }

    #[test]
    fn shard_record_rejects_crc_mismatch() {
        let shard = b"hello shard";
        let mut record = encode_shard_record(shard);
        record[5] ^= 0xFF;

        let decoded = decode_shard_record(&record, shard.len()).expect("decode");
        assert!(decoded.is_none());
    }

    #[test]
    fn shard_record_rejects_invalid_record_length() {
        let err = decode_shard_record(&[0_u8; 3], 1).expect_err("must fail");
        assert!(matches!(err, Error::InvalidFormat(_)));
    }
}
