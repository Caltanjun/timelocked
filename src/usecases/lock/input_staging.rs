//! Resolves lock input and output paths, then stages plaintext bytes for payload writing.

use std::fs::File;
use std::io::{BufReader, BufWriter, Seek, Write};
use std::path::{Path, PathBuf};

use tempfile::NamedTempFile;

use crate::base::{ensure_not_cancelled, CancellationToken, Error, Result};
use crate::domains::timelocked_file::{
    default_timelocked_output_path, ensure_timelocked_extension,
};

#[derive(Debug, Clone)]
enum InputSource {
    Stdin,
    File(PathBuf),
    Text(String),
}

impl InputSource {
    fn default_output_path(&self) -> Option<PathBuf> {
        match self {
            Self::Stdin | Self::Text(_) => None,
            Self::File(path) => Some(default_timelocked_output_path(path)),
        }
    }
}

#[derive(Debug)]
pub(super) struct StagedLockInput {
    pub(super) output_path: PathBuf,
    pub(super) output_parent: PathBuf,
    pub(super) plaintext_staging: NamedTempFile,
    pub(super) plaintext_bytes: u64,
    pub(super) original_filename: Option<String>,
}

pub(super) fn resolve_and_stage_input(
    input: &str,
    explicit_output: Option<PathBuf>,
    cancellation: Option<&CancellationToken>,
) -> Result<StagedLockInput> {
    let input_source = resolve_input_source(input);
    let output_path = resolve_lock_output_path(&input_source, explicit_output)?;
    assert_output_not_exists(&output_path)?;
    ensure_parent_dir_exists(&output_path)?;
    ensure_not_cancelled(cancellation)?;

    let output_parent = output_path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let mut plaintext_staging = NamedTempFile::new_in(&output_parent)?;
    let (plaintext_bytes, original_filename) =
        copy_input_to_staging(&input_source, &mut plaintext_staging)?;
    ensure_not_cancelled(cancellation)?;

    Ok(StagedLockInput {
        output_path,
        output_parent,
        plaintext_staging,
        plaintext_bytes,
        original_filename,
    })
}

fn resolve_input_source(input: &str) -> InputSource {
    if input == "-" {
        return InputSource::Stdin;
    }

    let input_path = Path::new(input);
    if input_path.exists() {
        return InputSource::File(input_path.to_path_buf());
    }

    InputSource::Text(input.to_string())
}

fn resolve_lock_output_path(
    input_source: &InputSource,
    explicit: Option<PathBuf>,
) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(ensure_timelocked_extension(path));
    }

    input_source.default_output_path().ok_or_else(|| {
        Error::InvalidArgument(
            "an explicit output path is required when locking stdin or text input".to_string(),
        )
    })
}

fn assert_output_not_exists(path: &Path) -> Result<()> {
    if path.exists() {
        return Err(Error::OutputExists(path.to_path_buf()));
    }
    Ok(())
}

fn ensure_parent_dir_exists(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn copy_input_to_staging(
    input: &InputSource,
    staging: &mut NamedTempFile,
) -> Result<(u64, Option<String>)> {
    let mut writer = BufWriter::new(staging.as_file_mut());

    match input {
        InputSource::Stdin => {
            let stdin = std::io::stdin();
            let mut handle = stdin.lock();
            let copied = std::io::copy(&mut handle, &mut writer)?;
            writer.flush()?;
            drop(writer);
            staging.as_file_mut().rewind()?;
            Ok((copied, None))
        }
        InputSource::File(input_path) => {
            let mut reader = BufReader::new(File::open(input_path)?);
            let copied = std::io::copy(&mut reader, &mut writer)?;
            writer.flush()?;
            drop(writer);
            staging.as_file_mut().rewind()?;

            let original_filename = input_path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.to_string());

            Ok((copied, original_filename))
        }
        InputSource::Text(text) => {
            writer.write_all(text.as_bytes())?;
            writer.flush()?;
            drop(writer);
            staging.as_file_mut().rewind()?;
            Ok((text.len() as u64, None))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{Read, Seek};
    use std::path::PathBuf;

    use tempfile::{tempdir, NamedTempFile};

    use crate::base::{CancellationToken, Error};

    use super::{
        assert_output_not_exists, copy_input_to_staging, ensure_parent_dir_exists,
        resolve_and_stage_input, resolve_input_source, resolve_lock_output_path, InputSource,
    };

    #[test]
    fn resolve_input_source_detects_existing_file() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("note.txt");
        fs::write(&path, b"hello").expect("write file");

        let source = resolve_input_source(path.to_str().expect("path utf8"));
        assert!(matches!(source, InputSource::File(_)));
    }

    #[test]
    fn resolve_input_source_treats_missing_path_as_text() {
        let source = resolve_input_source("hello future");
        assert!(matches!(source, InputSource::Text(_)));
    }

    #[test]
    fn resolve_input_source_detects_stdin_marker() {
        let source = resolve_input_source("-");
        assert!(matches!(source, InputSource::Stdin));
    }

    #[test]
    fn resolve_lock_output_path_requires_explicit_path_for_text_input() {
        let err = resolve_lock_output_path(&InputSource::Text("hello".to_string()), None)
            .expect_err("must fail");

        assert!(matches!(err, Error::InvalidArgument(_)));
        assert!(err.to_string().contains("explicit output path is required"));
    }

    #[test]
    fn resolve_lock_output_path_derives_default_for_file_input() {
        let resolved =
            resolve_lock_output_path(&InputSource::File(PathBuf::from("sample.txt")), None)
                .expect("resolve path");

        assert_eq!(resolved, PathBuf::from("sample.txt.timelocked"));
    }

    #[test]
    fn assert_output_not_exists_rejects_existing_file() {
        let dir = tempdir().expect("tempdir");
        let existing = dir.path().join("already.timelocked");
        fs::write(&existing, b"x").expect("write existing");

        let err = assert_output_not_exists(&existing).expect_err("must fail");
        assert!(matches!(err, Error::OutputExists(_)));
    }

    #[test]
    fn ensure_parent_dir_exists_creates_nested_directories() {
        let dir = tempdir().expect("tempdir");
        let output = dir
            .path()
            .join("nested")
            .join("deeper")
            .join("out.timelocked");

        ensure_parent_dir_exists(&output).expect("must create parent dirs");
        assert!(output.parent().expect("parent exists").exists());
    }

    #[test]
    fn copy_input_to_staging_supports_text_and_file_sources() {
        let dir = tempdir().expect("tempdir");

        let mut text_staging = NamedTempFile::new_in(dir.path()).expect("staging");
        let (text_len, text_name) =
            copy_input_to_staging(&InputSource::Text("hello".to_string()), &mut text_staging)
                .expect("copy text");
        assert_eq!(text_len, 5);
        assert_eq!(text_name, None);
        text_staging.as_file_mut().rewind().expect("rewind");
        let mut text_bytes = Vec::new();
        text_staging
            .as_file_mut()
            .read_to_end(&mut text_bytes)
            .expect("read staging");
        assert_eq!(text_bytes, b"hello");

        let input_file = dir.path().join("note.txt");
        fs::write(&input_file, b"from file").expect("write input file");
        let mut file_staging = NamedTempFile::new_in(dir.path()).expect("staging");
        let (file_len, file_name) =
            copy_input_to_staging(&InputSource::File(input_file.clone()), &mut file_staging)
                .expect("copy file");
        assert_eq!(file_len, 9);
        assert_eq!(file_name.as_deref(), Some("note.txt"));
        file_staging.as_file_mut().rewind().expect("rewind");
        let mut file_bytes = Vec::new();
        file_staging
            .as_file_mut()
            .read_to_end(&mut file_bytes)
            .expect("read staging");
        assert_eq!(file_bytes, b"from file");
    }

    #[test]
    fn resolve_and_stage_input_returns_cancelled_before_staging_begins() {
        let dir = tempdir().expect("tempdir");
        let output_path = dir.path().join("payload.timelocked");
        let cancellation = CancellationToken::default();
        cancellation.cancel();

        let err = resolve_and_stage_input(
            "hello staged future",
            Some(output_path.clone()),
            Some(&cancellation),
        )
        .expect_err("must cancel");

        assert!(matches!(err, Error::Cancelled));
        assert!(!output_path.exists());
        assert_eq!(fs::read_dir(dir.path()).expect("read dir").count(), 0);
    }
}
