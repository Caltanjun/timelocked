//! Reed-Solomon-protected payload-region encoding and reconstruction.

use crate::base::{Error, Result};

use super::reed_solomon::{encode_shards, reconstruct_shards};
use super::{decode_shard_record, encode_shard_record};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PayloadRegionLayout {
    pub rs_data_shards: u16,
    pub rs_parity_shards: u16,
    pub rs_shard_bytes: u32,
}

impl PayloadRegionLayout {
    pub fn total_shards(self) -> usize {
        self.rs_data_shards as usize + self.rs_parity_shards as usize
    }

    pub fn data_bytes_per_stripe(self) -> usize {
        self.rs_data_shards as usize * self.rs_shard_bytes as usize
    }

    pub fn shard_record_len(self) -> usize {
        self.rs_shard_bytes as usize + 4
    }

    pub fn stripe_record_len(self) -> usize {
        self.total_shards() * self.shard_record_len()
    }

    pub fn validate(self) -> Result<()> {
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
        Ok(())
    }
}

pub fn encode_payload_region(
    protected_stream: &[u8],
    layout: PayloadRegionLayout,
) -> Result<Vec<u8>> {
    layout.validate()?;

    let shard_bytes = layout.rs_shard_bytes as usize;
    let data_shards = layout.rs_data_shards as usize;
    let parity_shards = layout.rs_parity_shards as usize;
    let data_bytes_per_stripe = layout.data_bytes_per_stripe();
    let stripe_count = stripe_count(protected_stream.len(), data_bytes_per_stripe);

    let mut out = Vec::with_capacity(stripe_count * layout.stripe_record_len());
    for stripe_index in 0..stripe_count {
        let stripe_start = stripe_index * data_bytes_per_stripe;
        let stripe_end = protected_stream
            .len()
            .min(stripe_start + data_bytes_per_stripe);
        let stripe_bytes = &protected_stream[stripe_start..stripe_end];

        let mut shards = Vec::with_capacity(data_shards + parity_shards);
        for shard_index in 0..data_shards {
            let shard_start = shard_index * shard_bytes;
            let shard_end = stripe_bytes.len().min(shard_start + shard_bytes);
            let mut shard = vec![0_u8; shard_bytes];
            if shard_start < stripe_bytes.len() {
                shard[..shard_end - shard_start]
                    .copy_from_slice(&stripe_bytes[shard_start..shard_end]);
            }
            shards.push(shard);
        }
        for _ in 0..parity_shards {
            shards.push(vec![0_u8; shard_bytes]);
        }

        let encoded = encode_shards(data_shards, parity_shards, shards)?;
        for shard in encoded {
            out.extend_from_slice(&encode_shard_record(&shard));
        }
    }

    Ok(out)
}

pub fn reconstruct_payload_region(
    payload_region: &[u8],
    protected_stream_len: u64,
    layout: PayloadRegionLayout,
) -> Result<Vec<u8>> {
    layout.validate()?;

    let stripe_record_len = layout.stripe_record_len();
    if stripe_record_len == 0 || !payload_region.len().is_multiple_of(stripe_record_len) {
        return Err(Error::Verification(
            "payload region length mismatch".to_string(),
        ));
    }

    let shard_record_len = layout.shard_record_len();
    let shard_bytes = layout.rs_shard_bytes as usize;
    let data_shards = layout.rs_data_shards as usize;
    let parity_shards = layout.rs_parity_shards as usize;

    let mut recovered = Vec::with_capacity(payload_region.len());
    for stripe_bytes in payload_region.chunks_exact(stripe_record_len) {
        let mut shards = Vec::with_capacity(layout.total_shards());
        for record_bytes in stripe_bytes.chunks_exact(shard_record_len) {
            shards.push(decode_shard_record(record_bytes, shard_bytes)?);
        }

        reconstruct_shards(data_shards, parity_shards, &mut shards)?;

        for shard in shards.into_iter().take(data_shards) {
            recovered.extend_from_slice(shard.as_ref().ok_or_else(|| {
                Error::Verification("missing reconstructed data shard".to_string())
            })?);
        }
    }

    let protected_stream_len = protected_stream_len as usize;
    if recovered.len() < protected_stream_len {
        return Err(Error::Verification(
            "reconstructed protected stream is shorter than expected".to_string(),
        ));
    }
    recovered.truncate(protected_stream_len);
    Ok(recovered)
}

pub fn encoded_payload_region_len(
    protected_stream_len: u64,
    layout: PayloadRegionLayout,
) -> Result<u64> {
    layout.validate()?;
    let stripe_count = stripe_count(
        protected_stream_len as usize,
        layout.data_bytes_per_stripe(),
    ) as u64;
    Ok(stripe_count * layout.stripe_record_len() as u64)
}

fn stripe_count(total_len: usize, data_bytes_per_stripe: usize) -> usize {
    if total_len == 0 {
        1
    } else {
        total_len.div_ceil(data_bytes_per_stripe)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_layout() -> PayloadRegionLayout {
        PayloadRegionLayout {
            rs_data_shards: 3,
            rs_parity_shards: 2,
            rs_shard_bytes: 8,
        }
    }

    #[test]
    fn payload_region_round_trips_single_stripe() {
        let protected_stream = b"hello world";
        let layout = sample_layout();
        let encoded = encode_payload_region(protected_stream, layout).expect("encode");
        let recovered = reconstruct_payload_region(&encoded, protected_stream.len() as u64, layout)
            .expect("reconstruct");
        assert_eq!(recovered, protected_stream);
    }

    #[test]
    fn payload_region_round_trips_multiple_stripes() {
        let protected_stream = vec![7_u8; 60];
        let layout = sample_layout();
        let encoded = encode_payload_region(&protected_stream, layout).expect("encode");
        let recovered = reconstruct_payload_region(&encoded, protected_stream.len() as u64, layout)
            .expect("reconstruct");
        assert_eq!(recovered, protected_stream);
    }

    #[test]
    fn payload_region_trims_last_stripe_padding_back_to_protected_stream_len() {
        let protected_stream = b"trim me";
        let layout = sample_layout();
        let encoded = encode_payload_region(protected_stream, layout).expect("encode");
        let recovered = reconstruct_payload_region(&encoded, protected_stream.len() as u64, layout)
            .expect("reconstruct");
        assert_eq!(recovered.len(), protected_stream.len());
        assert_eq!(recovered, protected_stream);
    }

    #[test]
    fn payload_region_treats_crc_mismatch_as_erasure_and_recovers_within_parity_budget() {
        let protected_stream = vec![3_u8; 40];
        let layout = sample_layout();
        let mut encoded = encode_payload_region(&protected_stream, layout).expect("encode");

        encoded[10] ^= 0xFF;
        encoded[25] ^= 0x0F;

        let recovered = reconstruct_payload_region(&encoded, protected_stream.len() as u64, layout)
            .expect("reconstruct");
        assert_eq!(recovered, protected_stream);
    }

    #[test]
    fn payload_region_reconstruction_fails_when_erasures_exceed_parity_budget() {
        let protected_stream = vec![9_u8; 40];
        let layout = sample_layout();
        let mut encoded = encode_payload_region(&protected_stream, layout).expect("encode");
        let record_len = layout.shard_record_len();

        for index in 0..3 {
            let record_start = index * record_len;
            encoded[record_start + 4] ^= 0xFF;
        }

        let err = reconstruct_payload_region(&encoded, protected_stream.len() as u64, layout)
            .expect_err("must fail");
        assert!(matches!(err, Error::Verification(_)));
        assert!(err.to_string().contains("too damaged"));
    }
}
