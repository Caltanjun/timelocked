//! Test fixtures for building deterministic new-format `.timelocked` artifacts.

use std::fs::File;
use std::io::Cursor;
use std::path::Path;

use blake3::hash;
use num_bigint::BigUint;

use crate::base::Result;

use super::{
    choose_rs_shard_bytes_for_protected_stream_len, parse_container,
    predicted_protected_stream_len, write_timelocked_artifact, LockArtifactRequest,
    ParsedContainer, PayloadRegionEncodingParams, TimelockPayloadMaterial,
    DEFAULT_LOCK_CHUNK_SIZE_BYTES, DEFAULT_RS_DATA_SHARDS, DEFAULT_RS_PARITY_SHARDS,
};

#[derive(Debug, Clone)]
pub(crate) struct SampleTimelockedFileBuilder {
    plaintext: Vec<u8>,
    notice: Option<String>,
    original_filename: Option<String>,
    chunk_size: u32,
    iterations: u64,
    modulus_bits: u16,
    hardware_profile: String,
    target_seconds: Option<u64>,
    created_at_unix_seconds: u64,
    payload_region_params_override: Option<PayloadRegionEncodingParams>,
    file_key: [u8; 32],
    timelock_material: TimelockPayloadMaterial,
}

#[allow(dead_code)]
impl SampleTimelockedFileBuilder {
    pub(crate) fn new(plaintext: impl AsRef<[u8]>) -> Self {
        Self {
            plaintext: plaintext.as_ref().to_vec(),
            notice: None,
            original_filename: None,
            chunk_size: DEFAULT_LOCK_CHUNK_SIZE_BYTES as u32,
            iterations: 1,
            modulus_bits: 256,
            hardware_profile: "test-profile".to_string(),
            target_seconds: None,
            created_at_unix_seconds: 1_700_000_000,
            payload_region_params_override: None,
            file_key: [7_u8; 32],
            timelock_material: TimelockPayloadMaterial {
                modulus_n: BigUint::from(3233_u32),
                base_a: BigUint::from(5_u32),
                wrapped_key: [9_u8; 32],
            },
        }
    }

    pub(crate) fn notice(mut self, notice: impl Into<String>) -> Self {
        self.notice = Some(notice.into());
        self
    }

    pub(crate) fn original_filename(mut self, original_filename: impl Into<String>) -> Self {
        self.original_filename = Some(original_filename.into());
        self
    }

    pub(crate) fn chunk_size(mut self, chunk_size: u32) -> Self {
        self.chunk_size = chunk_size;
        self
    }

    pub(crate) fn iterations(mut self, iterations: u64) -> Self {
        self.iterations = iterations;
        self
    }

    pub(crate) fn modulus_bits(mut self, modulus_bits: u16) -> Self {
        self.modulus_bits = modulus_bits;
        self
    }

    pub(crate) fn hardware_profile(mut self, hardware_profile: impl Into<String>) -> Self {
        self.hardware_profile = hardware_profile.into();
        self
    }

    pub(crate) fn target_seconds(mut self, target_seconds: Option<u64>) -> Self {
        self.target_seconds = target_seconds;
        self
    }

    pub(crate) fn created_at_unix_seconds(mut self, created_at_unix_seconds: u64) -> Self {
        self.created_at_unix_seconds = created_at_unix_seconds;
        self
    }

    pub(crate) fn payload_region_params(
        mut self,
        rs_data_shards: u16,
        rs_parity_shards: u16,
        rs_shard_bytes: u32,
    ) -> Self {
        self.payload_region_params_override = Some(PayloadRegionEncodingParams {
            rs_data_shards,
            rs_parity_shards,
            rs_shard_bytes,
        });
        self
    }

    pub(crate) fn file_key(mut self, file_key: [u8; 32]) -> Self {
        self.file_key = file_key;
        self
    }

    pub(crate) fn timelock_material(mut self, timelock_material: TimelockPayloadMaterial) -> Self {
        self.timelock_material = timelock_material;
        self
    }

    pub(crate) fn write_to(&self, output_path: &Path) -> Result<()> {
        let mut writer = File::create(output_path)?;
        let protected_stream_len =
            predicted_protected_stream_len(self.plaintext.len() as u64, self.chunk_size)?;
        let payload_region_params =
            self.payload_region_params_override
                .unwrap_or(PayloadRegionEncodingParams {
                    rs_data_shards: DEFAULT_RS_DATA_SHARDS,
                    rs_parity_shards: DEFAULT_RS_PARITY_SHARDS,
                    rs_shard_bytes: choose_rs_shard_bytes_for_protected_stream_len(
                        protected_stream_len,
                    ),
                });
        let timelock_material = TimelockPayloadMaterial {
            modulus_n: self.timelock_material.modulus_n.clone(),
            base_a: self.timelock_material.base_a.clone(),
            wrapped_key: derive_wrapped_key(
                &self.file_key,
                &self.timelock_material.modulus_n,
                &self.timelock_material.base_a,
                self.iterations,
            ),
        };
        write_timelocked_artifact(
            &mut writer,
            &mut Cursor::new(&self.plaintext),
            &LockArtifactRequest {
                notice: self.notice.clone(),
                original_filename: self.original_filename.clone(),
                iterations: self.iterations,
                modulus_bits: self.modulus_bits,
                hardware_profile: self.hardware_profile.clone(),
                target_seconds: self.target_seconds,
                created_at_unix_seconds: self.created_at_unix_seconds,
                payload_plaintext_bytes: self.plaintext.len() as u64,
                aead_chunk_size_bytes: self.chunk_size,
                payload_region_params,
                key_bytes: self.file_key,
                timelock_material,
            },
            None,
            None,
        )
    }

    pub(crate) fn write_and_parse(&self, output_path: &Path) -> Result<ParsedContainer> {
        self.write_to(output_path)?;
        parse_container(output_path)
    }
}

fn derive_wrapped_key(
    file_key: &[u8; 32],
    modulus_n: &BigUint,
    base_a: &BigUint,
    iterations: u64,
) -> [u8; 32] {
    let mut value = base_a.clone();
    for _ in 0..iterations {
        value = (&value * &value) % modulus_n;
    }

    let digest = hash(&value.to_bytes_be());
    let mut wrapped_key = [0_u8; 32];
    for (index, byte) in wrapped_key.iter_mut().enumerate() {
        *byte = file_key[index] ^ digest.as_bytes()[index];
    }
    wrapped_key
}
