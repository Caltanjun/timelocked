//! Orchestrates the unlock flow: parse the container, recover the payload,
//! and return neutral recovered-payload data for the UI to render.

use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use tempfile::NamedTempFile;

use crate::base::progress_status::ProgressStatus;
use crate::base::{ensure_not_cancelled, CancellationToken, Error, Result};
use crate::domains::timelock::{unwrap_key_with_cancel, TimelockPuzzleMaterial};
use crate::domains::timelocked_file::{
    parse_container, recover_payload_to_writer_with_cancel, resolve_available_output_path,
    PayloadKind, RecoverFileKeyRequest,
};

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

    let mut recover_file_key = recover_file_key_from_timelock;

    match payload_kind {
        PayloadKind::Text => {
            let mut text_bytes = Vec::new();
            let stats = recover_payload_to_writer_with_cancel(
                &request.input,
                &parsed,
                &mut text_bytes,
                &mut recover_file_key,
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
                    &parsed,
                    &mut out_writer,
                    &mut recover_file_key,
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
    use std::fs;

    use tempfile::tempdir;

    use crate::base::Result;
    use crate::domains::timelock::create_puzzle_and_wrap_key;
    use crate::domains::timelocked_file::PayloadKind;
    use crate::domains::timelocked_file::{
        test_support::SampleTimelockedFileBuilder, TimelockPayloadMaterial,
    };

    use super::{
        execute, execute_with_cancel, resolve_recovered_file_path, RecoveredPayload, UnlockRequest,
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
