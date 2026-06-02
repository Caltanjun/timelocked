//! Orchestrates the unlock flow: parse the container, recover the payload,
//! and return neutral recovered-payload data for the UI to render.

use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use tempfile::NamedTempFile;

use crate::base::progress_status::ProgressStatus;
use zeroize::Zeroize;

use crate::base::{ensure_not_cancelled, CancellationToken, Error, Result, SecretString};
use crate::domains::timelock::{
    solve_puzzle_mask_with_cancel, unwrap_key_with_cancel, TimelockPuzzleMaterial,
};
use crate::domains::timelocked_file::{
    parse_container, recover_payload_to_writer_with_cancel, resolve_available_output_path,
    unwrap_file_key_with_password, PasswordProtectionMetadata, PayloadKind, RecoverFileKeyFn,
    RecoverFileKeyRequest,
};

pub const MAX_PASSWORD_ATTEMPTS: u8 = 3;

#[derive(Debug, Clone)]
pub struct UnlockRequest {
    pub input: PathBuf,
    pub out_dir: Option<PathBuf>,
    pub out: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct UnlockResponse {
    pub recovered_payload: RecoveredPayload,
    pub recovered_bytes: u64,
}

#[derive(Debug, Clone)]
pub enum RecoveredPayload {
    File { path: PathBuf },
    Text { text: String },
}

#[derive(Debug, Clone)]
pub struct PasswordPromptContext {
    pub input: PathBuf,
    pub attempt: u8,
    pub max_attempts: u8,
    pub previous_attempt_failed: bool,
}

pub type PasswordProvider<'a> = dyn FnMut(PasswordPromptContext) -> Result<SecretString> + 'a;

fn recover_file_key_from_timelock(
    request: RecoverFileKeyRequest<'_>,
    on_progress: Option<&mut dyn FnMut(ProgressStatus)>,
    cancellation: Option<&CancellationToken>,
) -> Result<[u8; 32]> {
    let puzzle = TimelockPuzzleMaterial {
        modulus_n: request.material.modulus_n.clone(),
        base_a: request.material.base_a.clone(),
        wrapped_key: request.material.wrapped_key,
        iterations: request.iterations,
        modulus_bits: request.modulus_bits,
    };

    unwrap_key_with_cancel(&puzzle, on_progress, cancellation)
}

pub fn execute(
    request: UnlockRequest,
    on_progress: Option<&mut dyn FnMut(ProgressStatus)>,
) -> Result<UnlockResponse> {
    execute_with_cancel(request, on_progress, None)
}

pub fn execute_with_cancel(
    request: UnlockRequest,
    on_progress: Option<&mut dyn FnMut(ProgressStatus)>,
    cancellation: Option<&CancellationToken>,
) -> Result<UnlockResponse> {
    execute_with_cancel_and_password_provider(request, on_progress, cancellation, None)
}

pub fn execute_with_cancel_and_password_provider(
    request: UnlockRequest,
    on_progress: Option<&mut dyn FnMut(ProgressStatus)>,
    cancellation: Option<&CancellationToken>,
    mut password_provider: Option<&mut PasswordProvider<'_>>,
) -> Result<UnlockResponse> {
    ensure_not_cancelled(cancellation)?;

    let mut noop_progress = |_event: ProgressStatus| {};
    let progress_cb: &mut dyn FnMut(ProgressStatus) = match on_progress {
        Some(cb) => cb,
        None => &mut noop_progress,
    };

    let parsed = parse_container(&request.input)?;
    let payload_kind = parsed.payload_kind();
    let recovered_file_path = resolve_recovered_file_path(&request, &payload_kind)?
        .map(|path| resolve_available_output_path(&path))
        .transpose()?;

    let password_protection = parsed.superblock.password_protection.clone();

    if password_protection.is_none() {
        let mut recover_file_key = recover_file_key_from_timelock;
        return recover_payload_once(
            &request,
            &parsed,
            payload_kind,
            recovered_file_path,
            &mut recover_file_key,
            progress_cb,
            cancellation,
        );
    }

    let password_protection = password_protection.expect("checked protected metadata");
    let mut cached_timelock_mask: Option<[u8; 32]> = None;
    let mut previous_attempt_failed = false;

    for attempt in 1..=MAX_PASSWORD_ATTEMPTS {
        let input = request.input.clone();
        let password_protection = password_protection.clone();
        let mut recover_file_key = |recover_request: RecoverFileKeyRequest<'_>,
                                    on_progress: Option<&mut dyn FnMut(ProgressStatus)>,
                                    cancellation: Option<&CancellationToken>|
         -> Result<[u8; 32]> {
            recover_password_protected_file_key(
                recover_request,
                &password_protection,
                &mut cached_timelock_mask,
                on_progress,
                cancellation,
                password_provider.as_deref_mut(),
                PasswordPromptContext {
                    input: input.clone(),
                    attempt,
                    max_attempts: MAX_PASSWORD_ATTEMPTS,
                    previous_attempt_failed,
                },
            )
        };

        match recover_payload_once(
            &request,
            &parsed,
            payload_kind.clone(),
            recovered_file_path.clone(),
            &mut recover_file_key,
            progress_cb,
            cancellation,
        ) {
            Ok(response) => {
                if let Some(mask) = cached_timelock_mask.as_mut() {
                    mask.zeroize();
                }
                return Ok(response);
            }
            Err(err)
                if is_payload_authentication_error(&err) && attempt < MAX_PASSWORD_ATTEMPTS =>
            {
                previous_attempt_failed = true;
            }
            Err(err) if is_payload_authentication_error(&err) => {
                if let Some(mask) = cached_timelock_mask.as_mut() {
                    mask.zeroize();
                }
                return Err(password_payload_authentication_error());
            }
            Err(err) => {
                if let Some(mask) = cached_timelock_mask.as_mut() {
                    mask.zeroize();
                }
                return Err(err);
            }
        }
    }

    if let Some(mask) = cached_timelock_mask.as_mut() {
        mask.zeroize();
    }
    Err(password_payload_authentication_error())
}

fn recover_payload_once(
    request: &UnlockRequest,
    parsed: &crate::domains::timelocked_file::ParsedContainer,
    payload_kind: PayloadKind,
    recovered_file_path: Option<PathBuf>,
    recover_file_key: &mut RecoverFileKeyFn<'_>,
    progress_cb: &mut dyn FnMut(ProgressStatus),
    cancellation: Option<&CancellationToken>,
) -> Result<UnlockResponse> {
    match payload_kind {
        PayloadKind::Text => {
            let mut text_bytes = Vec::new();
            let stats = recover_payload_to_writer_with_cancel(
                &request.input,
                parsed,
                &mut text_bytes,
                recover_file_key,
                Some(&mut *progress_cb),
                cancellation,
            )?;

            let text = String::from_utf8(text_bytes).map_err(|_| {
                Error::InvalidFormat("recovered text payload is not valid UTF-8".to_string())
            })?;

            Ok(UnlockResponse {
                recovered_payload: RecoveredPayload::Text { text },
                recovered_bytes: stats.plaintext_bytes,
            })
        }
        PayloadKind::File { .. } => {
            let output_path = recovered_file_path.ok_or_else(|| {
                Error::InvalidFormat(
                    "missing recovered file path for file payload unlock".to_string(),
                )
            })?;
            let output_parent = output_path.parent().unwrap_or(Path::new(".")).to_path_buf();
            std::fs::create_dir_all(&output_parent)?;
            let mut out_temp = NamedTempFile::new_in(&output_parent)?;
            let recovered_bytes;
            {
                let mut out_writer = BufWriter::new(out_temp.as_file_mut());
                let stats = recover_payload_to_writer_with_cancel(
                    &request.input,
                    parsed,
                    &mut out_writer,
                    recover_file_key,
                    Some(&mut *progress_cb),
                    cancellation,
                )?;
                out_writer.flush()?;
                recovered_bytes = stats.plaintext_bytes;
            }

            ensure_not_cancelled(cancellation)?;

            out_temp
                .persist(&output_path)
                .map_err(|err| Error::Io(err.error))?;

            Ok(UnlockResponse {
                recovered_payload: RecoveredPayload::File { path: output_path },
                recovered_bytes,
            })
        }
    }
}

fn recover_password_protected_file_key(
    request: RecoverFileKeyRequest<'_>,
    password_protection: &PasswordProtectionMetadata,
    cached_timelock_mask: &mut Option<[u8; 32]>,
    on_progress: Option<&mut dyn FnMut(ProgressStatus)>,
    cancellation: Option<&CancellationToken>,
    password_provider: Option<&mut PasswordProvider<'_>>,
    prompt_context: PasswordPromptContext,
) -> Result<[u8; 32]> {
    if cached_timelock_mask.is_none() {
        let puzzle = TimelockPuzzleMaterial {
            modulus_n: request.material.modulus_n.clone(),
            base_a: request.material.base_a.clone(),
            wrapped_key: request.material.wrapped_key,
            iterations: request.iterations,
            modulus_bits: request.modulus_bits,
        };
        *cached_timelock_mask = Some(solve_puzzle_mask_with_cancel(
            &puzzle,
            on_progress,
            cancellation,
        )?);
    }

    let provider = password_provider.ok_or_else(|| {
        Error::InvalidArgument("password required to unlock this file".to_string())
    })?;
    let mut passphrase = provider(prompt_context)?;
    let file_key = unwrap_file_key_with_password(
        &request.material.wrapped_key,
        cached_timelock_mask.as_ref().expect("timelock mask solved"),
        &passphrase,
        &password_protection.params,
    );
    passphrase.clear();
    file_key
}

fn is_payload_authentication_error(err: &Error) -> bool {
    matches!(err, Error::Crypto(message) if message.contains("payload authentication failed"))
}

fn password_payload_authentication_error() -> Error {
    Error::Crypto("payload authentication failed (wrong password or file is corrupted)".to_string())
}

fn resolve_recovered_file_path(
    request: &UnlockRequest,
    payload_kind: &PayloadKind,
) -> Result<Option<PathBuf>> {
    match payload_kind {
        PayloadKind::Text => {
            if request.out.is_some() || request.out_dir.is_some() {
                return Err(Error::InvalidArgument(
                    "output path options cannot be used when unlocking a text payload".to_string(),
                ));
            }

            Ok(None)
        }
        PayloadKind::File { original_filename } => {
            if let Some(path) = request.out.as_ref() {
                return Ok(Some(path.clone()));
            }

            let out_dir = request.out_dir.clone().unwrap_or_else(|| {
                request
                    .input
                    .parent()
                    .unwrap_or(Path::new("."))
                    .to_path_buf()
            });
            Ok(Some(out_dir.join(original_filename)))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::fs;
    use std::rc::Rc;

    use tempfile::tempdir;

    use crate::base::{Result, SecretString};
    use crate::domains::timelock::create_puzzle_and_wrap_key;
    use crate::domains::timelocked_file::PasswordProtectionParams;
    use crate::domains::timelocked_file::PayloadKind;
    use crate::domains::timelocked_file::{
        test_support::SampleTimelockedFileBuilder, TimelockPayloadMaterial,
    };

    use super::{
        execute, execute_with_cancel, execute_with_cancel_and_password_provider,
        resolve_recovered_file_path, PasswordPromptContext, RecoveredPayload, UnlockRequest,
    };
    use crate::base::{CancellationToken, Error};

    fn sample_timelocked_file_builder(
        plaintext: impl AsRef<[u8]>,
    ) -> Result<SampleTimelockedFileBuilder> {
        let file_key = [7_u8; 32];
        let puzzle = create_puzzle_and_wrap_key(&file_key, 1, 256)?;

        Ok(SampleTimelockedFileBuilder::new(plaintext)
            .file_key(file_key)
            .timelock_material(TimelockPayloadMaterial {
                modulus_n: puzzle.modulus_n,
                base_a: puzzle.base_a,
                wrapped_key: puzzle.wrapped_key,
            }))
    }

    fn protected_timelocked_file_builder(
        plaintext: impl AsRef<[u8]>,
    ) -> Result<SampleTimelockedFileBuilder> {
        Ok(sample_timelocked_file_builder(plaintext)?
            .password_protection(correct_passphrase(), test_password_params()))
    }

    fn correct_passphrase() -> SecretString {
        SecretString::new("correct horse battery staple".to_string())
    }

    fn wrong_passphrase() -> SecretString {
        SecretString::new("wrong password".to_string())
    }

    fn test_password_params() -> PasswordProtectionParams {
        PasswordProtectionParams::new(8, 1, 1, vec![1, 2, 3, 4, 5, 6, 7, 8])
            .expect("test password params")
    }

    #[test]
    fn execute_branches_between_message_and_file_outputs() {
        let dir = tempdir().expect("tempdir");
        let message_container = dir.path().join("message.timelocked");
        sample_timelocked_file_builder(b"hello future")
            .expect("builder")
            .chunk_size(4)
            .write_to(&message_container)
            .expect("message");

        let message_response = execute(
            UnlockRequest {
                input: message_container,
                out_dir: None,
                out: None,
            },
            None,
        )
        .expect("unlock message");

        assert_eq!(message_response.recovered_bytes, 12);
        assert!(matches!(
            message_response.recovered_payload,
            RecoveredPayload::Text { ref text } if text == "hello future"
        ));

        let file_container = dir.path().join("note.timelocked");
        sample_timelocked_file_builder(b"file payload")
            .expect("builder")
            .original_filename("note.txt")
            .chunk_size(4)
            .write_to(&file_container)
            .expect("file");

        let file_response = execute(
            UnlockRequest {
                input: file_container,
                out_dir: None,
                out: None,
            },
            None,
        )
        .expect("unlock file");

        match file_response.recovered_payload {
            RecoveredPayload::File { path } => {
                assert_eq!(path, dir.path().join("note.txt"));
                assert_eq!(fs::read(&path).expect("read output"), b"file payload");
            }
            other => panic!("expected file output, got {other:?}"),
        }
    }

    #[test]
    fn execute_rejects_invalid_utf8_message_payload() {
        let dir = tempdir().expect("tempdir");
        let container = dir.path().join("message.timelocked");
        sample_timelocked_file_builder([0xFF, 0xFE])
            .expect("builder")
            .chunk_size(4)
            .write_to(&container)
            .expect("container");

        let err = execute(
            UnlockRequest {
                input: container,
                out_dir: None,
                out: None,
            },
            None,
        )
        .expect_err("must fail");

        assert!(matches!(err, Error::InvalidFormat(_)));
        assert!(err
            .to_string()
            .contains("recovered text payload is not valid UTF-8"));
    }

    #[test]
    fn execute_suffixes_output_when_target_exists() {
        let dir = tempdir().expect("tempdir");
        let container = dir.path().join("note.timelocked");
        let existing_output = dir.path().join("note.txt");
        fs::write(&existing_output, b"existing").expect("write existing output");
        sample_timelocked_file_builder(b"recovered")
            .expect("builder")
            .original_filename("note.txt")
            .chunk_size(4)
            .write_to(&container)
            .expect("container");

        let response = execute(
            UnlockRequest {
                input: container,
                out_dir: None,
                out: None,
            },
            None,
        )
        .expect("unlock file");

        match response.recovered_payload {
            RecoveredPayload::File { path } => {
                assert_eq!(path, dir.path().join("note.1.txt"));
                assert_eq!(
                    fs::read(&existing_output).expect("read existing"),
                    b"existing"
                );
                assert_eq!(fs::read(&path).expect("read recovered"), b"recovered");
            }
            other => panic!("expected file output, got {other:?}"),
        }
    }

    #[test]
    fn execute_persists_only_final_output_file() {
        let dir = tempdir().expect("tempdir");
        let out_dir = dir.path().join("recovered");
        let container = dir.path().join("note.timelocked");
        sample_timelocked_file_builder(b"persist me")
            .expect("builder")
            .original_filename("note.txt")
            .chunk_size(4)
            .write_to(&container)
            .expect("container");

        let response = execute(
            UnlockRequest {
                input: container,
                out_dir: Some(out_dir.clone()),
                out: None,
            },
            None,
        )
        .expect("unlock file");

        let output_path = match response.recovered_payload {
            RecoveredPayload::File { path } => path,
            other => panic!("expected file output, got {other:?}"),
        };
        let entries = fs::read_dir(&out_dir)
            .expect("read output dir")
            .map(|entry| entry.expect("dir entry").path())
            .collect::<Vec<_>>();

        assert_eq!(entries, vec![output_path.clone()]);
        assert_eq!(fs::read(output_path).expect("read output"), b"persist me");
    }

    #[test]
    fn unlock_unprotected_v1_does_not_request_password() {
        let dir = tempdir().expect("tempdir");
        let container = dir.path().join("message.timelocked");
        sample_timelocked_file_builder(b"hello future")
            .expect("builder")
            .write_to(&container)
            .expect("container");
        let prompt_count = Rc::new(Cell::new(0));
        let provider_prompt_count = Rc::clone(&prompt_count);
        let mut provider = move |_context: PasswordPromptContext| {
            provider_prompt_count.set(provider_prompt_count.get() + 1);
            Ok(correct_passphrase())
        };

        let response = execute_with_cancel_and_password_provider(
            UnlockRequest {
                input: container,
                out_dir: None,
                out: None,
            },
            None,
            None,
            Some(&mut provider),
        )
        .expect("unlock unprotected");

        assert_eq!(prompt_count.get(), 0);
        assert!(
            matches!(response.recovered_payload, RecoveredPayload::Text { ref text } if text == "hello future")
        );
    }

    #[test]
    fn unlock_protected_requests_password_after_timelock_progress() {
        let dir = tempdir().expect("tempdir");
        let container = dir.path().join("message.timelocked");
        protected_timelocked_file_builder(b"hello")
            .expect("builder")
            .write_to(&container)
            .expect("container");
        let timelock_progress_count = Rc::new(Cell::new(0));
        let progress_timelock_count = Rc::clone(&timelock_progress_count);
        let saw_timelock_before_prompt = Rc::new(Cell::new(false));
        let provider_saw_timelock = Rc::clone(&saw_timelock_before_prompt);
        let provider_timelock_count = Rc::clone(&timelock_progress_count);
        let mut on_progress = move |status: crate::base::progress_status::ProgressStatus| {
            if status.phase == "unlock-timelock" {
                progress_timelock_count.set(progress_timelock_count.get() + 1);
            }
        };
        let mut provider = move |_context: PasswordPromptContext| {
            provider_saw_timelock.set(provider_timelock_count.get() > 0);
            Ok(correct_passphrase())
        };

        execute_with_cancel_and_password_provider(
            UnlockRequest {
                input: container,
                out_dir: None,
                out: None,
            },
            Some(&mut on_progress),
            None,
            Some(&mut provider),
        )
        .expect("unlock protected");

        assert!(saw_timelock_before_prompt.get());
    }

    #[test]
    fn unlock_protected_with_correct_password_recovers_text_payload() {
        let dir = tempdir().expect("tempdir");
        let container = dir.path().join("message.timelocked");
        protected_timelocked_file_builder(b"protected text")
            .expect("builder")
            .write_to(&container)
            .expect("container");
        let mut provider = |_context: PasswordPromptContext| Ok(correct_passphrase());

        let response = execute_with_cancel_and_password_provider(
            UnlockRequest {
                input: container,
                out_dir: None,
                out: None,
            },
            None,
            None,
            Some(&mut provider),
        )
        .expect("unlock protected text");

        assert!(
            matches!(response.recovered_payload, RecoveredPayload::Text { ref text } if text == "protected text")
        );
    }

    #[test]
    fn unlock_protected_with_correct_password_recovers_file_payload() {
        let dir = tempdir().expect("tempdir");
        let container = dir.path().join("note.timelocked");
        protected_timelocked_file_builder(b"protected file")
            .expect("builder")
            .original_filename("note.txt")
            .write_to(&container)
            .expect("container");
        let mut provider = |_context: PasswordPromptContext| Ok(correct_passphrase());

        let response = execute_with_cancel_and_password_provider(
            UnlockRequest {
                input: container,
                out_dir: None,
                out: None,
            },
            None,
            None,
            Some(&mut provider),
        )
        .expect("unlock protected file");

        match response.recovered_payload {
            RecoveredPayload::File { path } => {
                assert_eq!(fs::read(path).expect("read recovered"), b"protected file");
            }
            other => panic!("expected file output, got {other:?}"),
        }
    }

    #[test]
    fn unlock_protected_reprompts_after_wrong_password_without_resolving_timelock() {
        let dir = tempdir().expect("tempdir");
        let container = dir.path().join("message.timelocked");
        protected_timelocked_file_builder(b"protected")
            .expect("builder")
            .write_to(&container)
            .expect("container");
        let timelock_progress_count = Rc::new(Cell::new(0));
        let progress_timelock_count = Rc::clone(&timelock_progress_count);
        let mut on_progress = move |status: crate::base::progress_status::ProgressStatus| {
            if status.phase == "unlock-timelock" {
                progress_timelock_count.set(progress_timelock_count.get() + 1);
            }
        };
        let prompts = Rc::new(RefCell::new(vec![correct_passphrase(), wrong_passphrase()]));
        let attempts = Rc::new(RefCell::new(Vec::new()));
        let provider_prompts = Rc::clone(&prompts);
        let provider_attempts = Rc::clone(&attempts);
        let mut provider = move |context: PasswordPromptContext| {
            provider_attempts
                .borrow_mut()
                .push((context.attempt, context.previous_attempt_failed));
            Ok(provider_prompts.borrow_mut().pop().expect("prompt value"))
        };

        execute_with_cancel_and_password_provider(
            UnlockRequest {
                input: container,
                out_dir: None,
                out: None,
            },
            Some(&mut on_progress),
            None,
            Some(&mut provider),
        )
        .expect("unlock after retry");

        assert_eq!(*attempts.borrow(), vec![(1, false), (2, true)]);
        assert_eq!(timelock_progress_count.get(), 1);
    }

    #[test]
    fn unlock_protected_with_wrong_then_correct_password_recovers_payload() {
        let dir = tempdir().expect("tempdir");
        let container = dir.path().join("message.timelocked");
        protected_timelocked_file_builder(b"retry success")
            .expect("builder")
            .write_to(&container)
            .expect("container");
        let prompts = Rc::new(RefCell::new(vec![correct_passphrase(), wrong_passphrase()]));
        let provider_prompts = Rc::clone(&prompts);
        let mut provider = move |_context: PasswordPromptContext| {
            Ok(provider_prompts.borrow_mut().pop().expect("prompt"))
        };

        let response = execute_with_cancel_and_password_provider(
            UnlockRequest {
                input: container,
                out_dir: None,
                out: None,
            },
            None,
            None,
            Some(&mut provider),
        )
        .expect("unlock after wrong password");

        assert!(
            matches!(response.recovered_payload, RecoveredPayload::Text { ref text } if text == "retry success")
        );
    }

    #[test]
    fn unlock_protected_fails_after_three_wrong_password_attempts() {
        let dir = tempdir().expect("tempdir");
        let container = dir.path().join("message.timelocked");
        protected_timelocked_file_builder(b"protected")
            .expect("builder")
            .write_to(&container)
            .expect("container");
        let prompt_count = Rc::new(Cell::new(0));
        let provider_prompt_count = Rc::clone(&prompt_count);
        let mut provider = move |_context: PasswordPromptContext| {
            provider_prompt_count.set(provider_prompt_count.get() + 1);
            Ok(wrong_passphrase())
        };

        let err = execute_with_cancel_and_password_provider(
            UnlockRequest {
                input: container,
                out_dir: None,
                out: None,
            },
            None,
            None,
            Some(&mut provider),
        )
        .expect_err("wrong passwords fail");

        assert_eq!(prompt_count.get(), 3);
        assert!(matches!(err, Error::Crypto(_)));
        assert!(err
            .to_string()
            .contains("wrong password or file is corrupted"));
    }

    #[test]
    fn unlock_protected_without_password_provider_returns_password_required() {
        let dir = tempdir().expect("tempdir");
        let container = dir.path().join("message.timelocked");
        protected_timelocked_file_builder(b"protected")
            .expect("builder")
            .write_to(&container)
            .expect("container");

        let err = execute_with_cancel_and_password_provider(
            UnlockRequest {
                input: container,
                out_dir: None,
                out: None,
            },
            None,
            None,
            None,
        )
        .expect_err("missing password provider");

        assert!(matches!(err, Error::InvalidArgument(_)));
        assert!(err.to_string().contains("password required"));
    }

    #[test]
    fn unlock_protected_password_provider_cancel_returns_cancelled() {
        let dir = tempdir().expect("tempdir");
        let container = dir.path().join("message.timelocked");
        protected_timelocked_file_builder(b"protected")
            .expect("builder")
            .write_to(&container)
            .expect("container");
        let mut provider = |_context: PasswordPromptContext| Err(Error::Cancelled);

        let err = execute_with_cancel_and_password_provider(
            UnlockRequest {
                input: container,
                out_dir: None,
                out: None,
            },
            None,
            None,
            Some(&mut provider),
        )
        .expect_err("provider cancellation");

        assert!(matches!(err, Error::Cancelled));
    }

    #[test]
    fn unlock_output_suffixing_still_works_for_protected_file_payloads() {
        let dir = tempdir().expect("tempdir");
        let container = dir.path().join("note.timelocked");
        let existing_output = dir.path().join("note.txt");
        fs::write(&existing_output, b"existing").expect("write existing output");
        protected_timelocked_file_builder(b"protected recovered")
            .expect("builder")
            .original_filename("note.txt")
            .write_to(&container)
            .expect("container");
        let mut provider = |_context: PasswordPromptContext| Ok(correct_passphrase());

        let response = execute_with_cancel_and_password_provider(
            UnlockRequest {
                input: container,
                out_dir: None,
                out: None,
            },
            None,
            None,
            Some(&mut provider),
        )
        .expect("unlock protected file");

        match response.recovered_payload {
            RecoveredPayload::File { path } => {
                assert_eq!(path, dir.path().join("note.1.txt"));
                assert_eq!(
                    fs::read(existing_output).expect("read existing"),
                    b"existing"
                );
                assert_eq!(
                    fs::read(path).expect("read recovered"),
                    b"protected recovered"
                );
            }
            other => panic!("expected file output, got {other:?}"),
        }
    }

    #[test]
    fn execute_respects_cancellation_before_persist() {
        let dir = tempdir().expect("tempdir");
        let out_dir = dir.path().join("recovered");
        let container = dir.path().join("note.timelocked");
        sample_timelocked_file_builder(b"done")
            .expect("builder")
            .original_filename("note.txt")
            .chunk_size(4)
            .write_to(&container)
            .expect("container");

        let cancellation = CancellationToken::default();
        let cancel_handle = cancellation.clone();
        let mut on_progress = move |status: crate::base::progress_status::ProgressStatus| {
            if status.phase == "unlock-decrypt" {
                cancel_handle.cancel();
            }
        };

        let err = execute_with_cancel(
            UnlockRequest {
                input: container,
                out_dir: Some(out_dir.clone()),
                out: None,
            },
            Some(&mut on_progress),
            Some(&cancellation),
        )
        .expect_err("must cancel");

        assert!(matches!(err, Error::Cancelled));
        assert!(!out_dir.join("note.txt").exists());
        if out_dir.exists() {
            assert_eq!(fs::read_dir(&out_dir).expect("read dir").count(), 0);
        }
    }

    #[test]
    fn execute_respects_cancellation_during_recovery() {
        let dir = tempdir().expect("tempdir");
        let out_dir = dir.path().join("recovered");
        let container = dir.path().join("note.timelocked");
        sample_timelocked_file_builder(b"abcdef")
            .expect("builder")
            .original_filename("note.txt")
            .chunk_size(2)
            .write_to(&container)
            .expect("container");

        let cancellation = CancellationToken::default();
        let cancel_handle = cancellation.clone();
        let mut saw_first_decrypt = false;
        let mut on_progress = move |status: crate::base::progress_status::ProgressStatus| {
            if status.phase == "unlock-decrypt" && !saw_first_decrypt {
                saw_first_decrypt = true;
                cancel_handle.cancel();
            }
        };

        let err = execute_with_cancel(
            UnlockRequest {
                input: container,
                out_dir: Some(out_dir.clone()),
                out: None,
            },
            Some(&mut on_progress),
            Some(&cancellation),
        )
        .expect_err("must cancel");

        assert!(matches!(err, Error::Cancelled));
        assert!(!out_dir.join("note.txt").exists());
        if out_dir.exists() {
            assert_eq!(fs::read_dir(&out_dir).expect("read dir").count(), 0);
        }
    }

    #[test]
    fn resolve_recovered_file_path_rejects_output_options_for_text_payload() {
        let dir = tempdir().expect("tempdir");
        let container = dir.path().join("message.timelocked");
        let parsed =
            crate::domains::timelocked_file::test_support::SampleTimelockedFileBuilder::new(
                b"hello",
            )
            .write_and_parse(&container)
            .expect("parsed");

        let err = resolve_recovered_file_path(
            &UnlockRequest {
                input: container,
                out_dir: Some(dir.path().join("out")),
                out: None,
            },
            &parsed.payload_kind(),
        )
        .expect_err("must fail");

        assert!(matches!(err, Error::InvalidArgument(_)));
        assert!(err.to_string().contains("text payload"));
    }

    #[test]
    fn resolve_recovered_file_path_defaults_to_input_parent_and_original_filename() {
        let dir = tempdir().expect("tempdir");
        let input = dir.path().join("fixtures").join("archive.timelocked");
        fs::create_dir_all(input.parent().expect("parent")).expect("mkdir");
        let parsed =
            crate::domains::timelocked_file::test_support::SampleTimelockedFileBuilder::new(
                b"hello",
            )
            .original_filename("note.txt")
            .write_and_parse(&input)
            .expect("parsed");

        let resolved = resolve_recovered_file_path(
            &UnlockRequest {
                input,
                out_dir: None,
                out: None,
            },
            &parsed.payload_kind(),
        )
        .expect("resolve path");

        assert_eq!(resolved, Some(dir.path().join("fixtures").join("note.txt")));
    }

    #[test]
    fn resolve_recovered_file_path_uses_explicit_out_for_file_payloads() {
        let dir = tempdir().expect("tempdir");
        let explicit_out = dir.path().join("custom.txt");

        let resolved = resolve_recovered_file_path(
            &UnlockRequest {
                input: dir.path().join("archive.timelocked"),
                out_dir: Some(dir.path().join("ignored")),
                out: Some(explicit_out.clone()),
            },
            &PayloadKind::File {
                original_filename: "note.txt".to_string(),
            },
        )
        .expect("resolve path");

        assert_eq!(resolved, Some(explicit_out));
    }
}
