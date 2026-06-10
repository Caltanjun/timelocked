//! Orchestrates the lock flow by staging input, building payload material,
//! writing the container, and optionally verifying the final output.

mod calibration;
mod input_staging;
mod output_verification;
mod payload_writer;
mod persistence;

use std::{fmt, path::PathBuf};

use crate::base::progress_status::ProgressStatus;
use crate::base::{ensure_not_cancelled, CancellationToken, Result, SecretString};
use crate::domains::timelock::resolve_lock_difficulty;

use calibration::resolve_current_machine_iterations_per_second;
use input_staging::resolve_and_stage_input;
use output_verification::verify_output_if_requested;
use payload_writer::write_payload_artifacts;
use persistence::persist_timelocked_container;

#[derive(Clone)]
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
    pub password: Option<SecretString>,
    pub verify: bool,
}

impl fmt::Debug for LockRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LockRequest")
            .field("input", &self.input)
            .field("output", &self.output)
            .field("modulus_bits", &self.modulus_bits)
            .field("target", &self.target)
            .field("iterations", &self.iterations)
            .field("hardware_profile", &self.hardware_profile)
            .field(
                "current_machine_iterations_per_second",
                &self.current_machine_iterations_per_second,
            )
            .field("creator_name", &self.creator_name)
            .field("creator_message", &self.creator_message)
            .field("password", &self.password.as_ref().map(|_| "[REDACTED]"))
            .field("verify", &self.verify)
            .finish()
    }
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

    use crate::base::{CancellationToken, Error, SecretString};
    use crate::domains::timelocked_file::{parse_container, BODY_VERSION_V1, BODY_VERSION_V2};
    use crate::usecases::{inspect, verify};

    use super::{execute_with_cancel, LockRequest};

    #[test]
    fn lock_without_password_writes_v1_artifact() {
        let dir = tempdir().expect("tempdir");
        let output_path = dir.path().join("without-password.timelocked");

        execute_with_cancel(lock_request("hello v1", &output_path, None), None, None)
            .expect("lock without password");

        let parsed = parse_container(&output_path).expect("parse output");
        assert_eq!(parsed.superblock.body_version, BODY_VERSION_V1);
        assert!(parsed.superblock.password_protection.is_none());
    }

    #[test]
    fn lock_with_password_writes_v2_artifact() {
        let dir = tempdir().expect("tempdir");
        let output_path = dir.path().join("with-password.timelocked");

        execute_with_cancel(
            lock_request(
                "hello v2",
                &output_path,
                Some(SecretString::new("keep this secret".to_string())),
            ),
            None,
            None,
        )
        .expect("lock with password");

        let parsed = parse_container(&output_path).expect("parse output");
        assert_eq!(parsed.superblock.body_version, BODY_VERSION_V2);
        assert!(parsed.superblock.password_protection.is_some());
    }

    #[test]
    fn lock_with_password_sets_password_protected_inspect_flag() {
        let dir = tempdir().expect("tempdir");
        let output_path = dir.path().join("inspect-password.timelocked");

        execute_with_cancel(
            lock_request(
                "inspect me",
                &output_path,
                Some(SecretString::new("password".to_string())),
            ),
            None,
            None,
        )
        .expect("lock with password");

        let response = inspect::execute(inspect::InspectRequest {
            input: output_path,
            current_machine_iterations_per_second: None,
        })
        .expect("inspect");

        assert!(response.header.password_protection.password_protected);
        assert_eq!(response.format_version, BODY_VERSION_V2);
    }

    #[test]
    fn lock_rejects_explicit_empty_password() {
        let dir = tempdir().expect("tempdir");
        let output_path = dir.path().join("empty-password.timelocked");

        let err = execute_with_cancel(
            lock_request(
                "reject me",
                &output_path,
                Some(SecretString::new(String::new())),
            ),
            None,
            None,
        )
        .expect_err("empty password must be rejected");

        assert!(matches!(err, Error::InvalidArgument(_)));
        assert!(!output_path.exists());
    }

    #[test]
    fn lock_with_password_and_verify_still_succeeds_structurally() {
        let dir = tempdir().expect("tempdir");
        let output_path = dir.path().join("verify-password.timelocked");

        execute_with_cancel(
            lock_request(
                "structural verification only",
                &output_path,
                Some(SecretString::new("password".to_string())),
            ),
            None,
            None,
        )
        .expect("lock with password");

        let response = verify::execute(
            verify::VerifyRequest {
                input: output_path.clone(),
            },
            None,
        )
        .expect("verify structurally");

        assert_eq!(response.path, output_path);
        assert_eq!(
            response.payload_plaintext_bytes,
            "structural verification only".len() as u64
        );
    }

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
                password: None,
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
                password: None,
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
                password: None,
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

    #[test]
    fn lock_request_debug_does_not_include_password() {
        let request = LockRequest {
            input: "hello".to_string(),
            output: None,
            modulus_bits: 256,
            target: None,
            iterations: Some(1),
            hardware_profile: None,
            current_machine_iterations_per_second: None,
            creator_name: None,
            creator_message: None,
            password: Some(SecretString::new("do not log me".to_string())),
            verify: false,
        };

        let debug = format!("{request:?}");

        assert!(debug.contains("password"));
        assert!(debug.contains("REDACTED"));
        assert!(!debug.contains("do not log me"));
    }

    fn lock_request(
        input: &str,
        output_path: &std::path::Path,
        password: Option<SecretString>,
    ) -> LockRequest {
        LockRequest {
            input: input.to_string(),
            output: Some(output_path.to_path_buf()),
            modulus_bits: 256,
            target: None,
            iterations: Some(1),
            hardware_profile: None,
            current_machine_iterations_per_second: None,
            creator_name: None,
            creator_message: None,
            password,
            verify: false,
        }
    }
}
