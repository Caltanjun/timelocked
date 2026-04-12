//! Small internal adapter around `reed-solomon-erasure`.

use crate::base::{Error, Result};

pub(crate) fn encode_shards(
    data_shards: usize,
    parity_shards: usize,
    mut shards: Vec<Vec<u8>>,
) -> Result<Vec<Vec<u8>>> {
    let codec = codec(data_shards, parity_shards)?;
    codec
        .encode(&mut shards)
        .map_err(|err| Error::InvalidFormat(format!("Reed-Solomon encode failed: {err}")))?;
    Ok(shards)
}

pub(crate) fn reconstruct_shards(
    data_shards: usize,
    parity_shards: usize,
    shards: &mut [Option<Vec<u8>>],
) -> Result<()> {
    let codec = codec(data_shards, parity_shards)?;
    if shards.len() != data_shards + parity_shards {
        return Err(Error::InvalidFormat(
            "invalid shard count for Reed-Solomon reconstruction".to_string(),
        ));
    }
    codec.reconstruct(shards).map_err(|err| match err {
        reed_solomon_erasure::Error::TooManyShards => {
            Error::InvalidFormat("unsupported total shard count".to_string())
        }
        reed_solomon_erasure::Error::TooFewShardsPresent => Error::Verification(
            "payload region is too damaged for Reed-Solomon recovery".to_string(),
        ),
        other => Error::InvalidFormat(format!("Reed-Solomon reconstruction failed: {other}")),
    })
}

fn codec(
    data_shards: usize,
    parity_shards: usize,
) -> Result<reed_solomon_erasure::galois_8::ReedSolomon> {
    reed_solomon_erasure::galois_8::ReedSolomon::new(data_shards, parity_shards).map_err(|err| {
        match err {
            reed_solomon_erasure::Error::TooManyShards => {
                Error::InvalidFormat("unsupported total shard count".to_string())
            }
            other => Error::InvalidFormat(format!("invalid Reed-Solomon parameters: {other}")),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{codec, encode_shards};

    #[test]
    fn reed_solomon_backend_rejects_unsupported_total_shard_count() {
        let err = codec(200, 100).expect_err("must fail");
        assert!(err.to_string().contains("unsupported total shard count"));
    }

    #[test]
    fn reed_solomon_backend_encodes_supported_shard_count() {
        let shards = vec![vec![1_u8; 4], vec![2_u8; 4], vec![0_u8; 4]];
        let encoded = encode_shards(2, 1, shards).expect("encode");
        assert_eq!(encoded.len(), 3);
        assert_eq!(encoded[0], vec![1_u8; 4]);
    }
}
