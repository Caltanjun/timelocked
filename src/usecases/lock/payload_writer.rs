//! Writes complete lock artifacts using the new domain file-format writer.

use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use rand::rngs::OsRng;
use rand::RngCore;
use tempfile::NamedTempFile;
use zeroize::Zeroize;

use crate::base::progress_status::ProgressStatus;
use crate::base::{ensure_not_cancelled, CancellationToken, Result};
use crate::domains::timelock::create_puzzle_and_wrap_key;
use crate::domains::timelocked_file::{
    choose_rs_shard_bytes_for_protected_stream_len, predicted_protected_stream_len,
    write_timelocked_artifact, LockArtifactRequest, PayloadRegionEncodingParams,
    TimelockPayloadMaterial, DEFAULT_LOCK_CHUNK_SIZE_BYTES, DEFAULT_RS_DATA_SHARDS,
    DEFAULT_RS_PARITY_SHARDS,
};

use super::input_staging::StagedLockInput;
use super::LockRequest;

struct PayloadWritePlan {
    iterations: u64,
    hardware_profile: String,
    target_seconds: Option<u64>,
}

pub(super) struct PayloadArtifacts {
    pub(super) artifact_temp: NamedTempFile,
}

pub(super) fn write_payload_artifacts(
    staged_input: &StagedLockInput,
    request: &LockRequest,
    iterations: u64,
    hardware_profile: String,
    target_seconds: Option<u64>,
    progress_cb: &mut dyn FnMut(ProgressStatus),
    cancellation: Option<&CancellationToken>,
) -> Result<PayloadArtifacts> {
    ensure_not_cancelled(cancellation)?;

    let plan = PayloadWritePlan {
        iterations,
        hardware_profile,
        target_seconds,
    };

    let mut file_key = [0_u8; 32];
    OsRng.fill_bytes(&mut file_key);

    let payload_result = write_payload_artifacts_with_key(
        staged_input,
        request,
        &plan,
        &mut file_key,
        progress_cb,
        cancellation,
    );
    file_key.zeroize();
    payload_result
}

fn write_payload_artifacts_with_key(
    staged_input: &StagedLockInput,
    request: &LockRequest,
    plan: &PayloadWritePlan,
    file_key: &mut [u8; 32],
    progress_cb: &mut dyn FnMut(ProgressStatus),
    cancellation: Option<&CancellationToken>,
) -> Result<PayloadArtifacts> {
    progress_cb(ProgressStatus::new("lock-primes", 0, 1, None, None));

    let puzzle = create_puzzle_and_wrap_key(file_key, plan.iterations, request.modulus_bits)?;

    progress_cb(ProgressStatus::new("lock-puzzle", 1, 1, None, None));

    let protected_stream_len = predicted_protected_stream_len(
        staged_input.plaintext_bytes,
        DEFAULT_LOCK_CHUNK_SIZE_BYTES as u32,
    )?;
    let rs_shard_bytes = choose_rs_shard_bytes_for_protected_stream_len(protected_stream_len);

    let mut artifact_temp = NamedTempFile::new_in(&staged_input.output_parent)?;
    {
        let mut output_writer = BufWriter::new(artifact_temp.as_file_mut());
        let mut plaintext_reader =
            BufReader::new(File::open(staged_input.plaintext_staging.path())?);
        write_timelocked_artifact(
            &mut output_writer,
            &mut plaintext_reader,
            &LockArtifactRequest {
                notice: None,
                original_filename: staged_input.original_filename.clone(),
                iterations: plan.iterations,
                modulus_bits: puzzle.modulus_bits,
                hardware_profile: plan.hardware_profile.clone(),
                target_seconds: plan.target_seconds,
                created_at_unix_seconds: current_unix_seconds(),
                payload_plaintext_bytes: staged_input.plaintext_bytes,
                aead_chunk_size_bytes: DEFAULT_LOCK_CHUNK_SIZE_BYTES as u32,
                payload_region_params: PayloadRegionEncodingParams {
                    rs_data_shards: DEFAULT_RS_DATA_SHARDS,
                    rs_parity_shards: DEFAULT_RS_PARITY_SHARDS,
                    rs_shard_bytes,
                },
                key_bytes: *file_key,
                timelock_material: TimelockPayloadMaterial {
                    modulus_n: puzzle.modulus_n,
                    base_a: puzzle.base_a,
                    wrapped_key: puzzle.wrapped_key,
                },
            },
            Some(progress_cb),
            cancellation,
        )?;
        output_writer.flush()?;
    }

    Ok(PayloadArtifacts { artifact_temp })
}

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
