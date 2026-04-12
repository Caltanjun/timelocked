//! Orchestrates the lock flow by staging input, building payload material,
//! writing the container, and optionally verifying the final output.

mod calibration;
mod input_staging;
mod output_verification;
mod payload_writer;
mod persistence;

use std::path::PathBuf;

use crate::base::progress_status::ProgressStatus;
use crate::base::{ensure_not_cancelled, CancellationToken, Result};
use crate::domains::timelock::resolve_lock_difficulty;

use calibration::resolve_current_machine_iterations_per_second;
use input_staging::resolve_and_stage_input;
use output_verification::verify_output_if_requested;
use payload_writer::write_payload_artifacts;
use persistence::persist_timelocked_container;

#[derive(Debug, Clone)]
pub struct LockRequest {
    pub input: String,
    pub output: Option<PathBuf>,
    pub modulus_bits: usize,
    pub target: Option<String>,
    pub iterations: Option<u64>,
    pub hardware_profile: Option<String>,
    pub current_machine_iterations_per_second: Option<u64>,
    pub creator_name: Option<String>,
    pub creator_message: Option<String>,
    pub verify: bool,
}

#[derive(Debug, Clone)]
pub struct LockResponse {
    pub output_path: PathBuf,
    pub iterations: u64,
    pub hardware_profile: String,
    pub payload_bytes: u64,
}

pub fn execute(
    request: LockRequest,
    on_progress: Option<&mut dyn FnMut(ProgressStatus)>,
) -> Result<LockResponse> {
    execute_with_cancel(request, on_progress, None)
}

pub fn execute_with_cancel(
    request: LockRequest,
    on_progress: Option<&mut dyn FnMut(ProgressStatus)>,
    cancellation: Option<&CancellationToken>,
) -> Result<LockResponse> {
    ensure_not_cancelled(cancellation)?;

    let mut noop_progress = |_event: ProgressStatus| {};
    let progress_cb: &mut dyn FnMut(ProgressStatus) = match on_progress {
        Some(cb) => cb,
        None => &mut noop_progress,
    };

    let staged_input =
        resolve_and_stage_input(&request.input, request.output.clone(), cancellation)?;
    let current_machine_iterations_per_second =
        resolve_current_machine_iterations_per_second(&request)?;
    let difficulty = resolve_lock_difficulty(
        request.iterations,
        request.target.as_deref(),
        request.hardware_profile.as_deref(),
        current_machine_iterations_per_second,
    )?;
    let payload_artifacts = write_payload_artifacts(
        &staged_input,
        &request,
        difficulty.iterations,
        difficulty.hardware_profile_id.clone(),
        difficulty.target_seconds,
        progress_cb,
        cancellation,
    )?;

    persist_timelocked_container(&staged_input.output_path, payload_artifacts.artifact_temp)?;

    verify_output_if_requested(request.verify, &staged_input.output_path, cancellation)?;

    Ok(LockResponse {
        output_path: staged_input.output_path,
        iterations: difficulty.iterations,
        hardware_profile: difficulty.hardware_profile_id,
        payload_bytes: staged_input.plaintext_bytes,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::base::{CancellationToken, Error};
    use crate::domains::timelocked_file::parse_container;

    use super::{execute_with_cancel, LockRequest};

    #[test]
    fn execute_with_cancel_supports_optional_verification() {
        let dir = tempdir().expect("tempdir");
        let output_path = dir.path().join("verified.timelocked");

        let response = execute_with_cancel(
            LockRequest {
                input: "hello verified future".to_string(),
                output: Some(output_path.clone()),
                modulus_bits: 256,
                target: None,
                iterations: Some(1),
                hardware_profile: None,
                current_machine_iterations_per_second: None,
                creator_name: Some("Marty".to_string()),
                creator_message: Some("See you later".to_string()),
                verify: true,
            },
            None,
            None,
        )
        .expect("lock with verify");

        assert_eq!(response.output_path, output_path);
        assert!(output_path.exists());

        let parsed = parse_container(&output_path).expect("parse output");
        assert_eq!(parsed.header.creator_name, None);
        assert_eq!(parsed.header.creator_message, None);
        assert_eq!(
            parsed.superblock.payload_plaintext_bytes,
            "hello verified future".len() as u64
        );
    }

    #[test]
    fn execute_with_cancel_keeps_only_final_output_after_success() {
        let dir = tempdir().expect("tempdir");
        let out_dir = dir.path().join("locked");
        let output_path = out_dir.join("payload.timelocked");

        let response = execute_with_cancel(
            LockRequest {
                input: "persist only the final file".to_string(),
                output: Some(output_path.clone()),
                modulus_bits: 256,
                target: None,
                iterations: Some(1),
                hardware_profile: None,
                current_machine_iterations_per_second: None,
                creator_name: None,
                creator_message: None,
                verify: false,
            },
            None,
            None,
        )
        .expect("lock output");

        let entries = fs::read_dir(&out_dir)
            .expect("read output dir")
            .map(|entry| entry.expect("dir entry").path())
            .collect::<Vec<_>>();

        assert_eq!(response.output_path, output_path);
        assert_eq!(entries, vec![output_path]);
    }

    #[test]
    fn execute_with_cancel_cleans_up_when_cancelled_during_encryption() {
        let dir = tempdir().expect("tempdir");
        let out_dir = dir.path().join("cancelled");
        let output_path = out_dir.join("payload.timelocked");
        let cancellation = CancellationToken::default();
        let cancel_handle = cancellation.clone();
        let mut saw_encrypt_progress = false;
        let mut on_progress = move |status: crate::base::progress_status::ProgressStatus| {
            if status.phase == "lock-encrypt" && !saw_encrypt_progress {
                saw_encrypt_progress = true;
                cancel_handle.cancel();
            }
        };

        let err = execute_with_cancel(
            LockRequest {
                input: "abcdef".repeat(400_000),
                output: Some(output_path.clone()),
                modulus_bits: 256,
                target: None,
                iterations: Some(1),
                hardware_profile: None,
                current_machine_iterations_per_second: None,
                creator_name: None,
                creator_message: None,
                verify: false,
            },
            Some(&mut on_progress),
            Some(&cancellation),
        )
        .expect_err("must cancel");

        assert!(matches!(err, Error::Cancelled));
        assert!(!output_path.exists());
        if out_dir.exists() {
            assert_eq!(fs::read_dir(&out_dir).expect("read dir").count(), 0);
        }
    }
}
