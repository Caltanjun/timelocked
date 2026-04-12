//! Collision-avoidance logic for output files.
//! Finds the next available filename (e.g., `file.1.txt`) if the target already exists.

use std::path::{Path, PathBuf};

use crate::base::{Error, Result};

pub fn resolve_available_output_path(path: &Path) -> Result<PathBuf> {
    if !path.exists() {
        return Ok(path.to_path_buf());
    }

    let file_stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .ok_or_else(|| Error::InvalidArgument("output filename is invalid".to_string()))?;
    let extension = path
        .extension()
        .map(|extension| extension.to_string_lossy().to_string());

    let (base_stem, start_index) = parse_suffix_index(&file_stem);
    let mut index = start_index;

    loop {
        let candidate_name = if let Some(ext) = extension.as_deref() {
            format!("{base_stem}.{index}.{ext}")
        } else {
            format!("{base_stem}.{index}")
        };
        let candidate = path.with_file_name(candidate_name);
        if !candidate.exists() {
            return Ok(candidate);
        }

        if index == u64::MAX {
            return Err(Error::InvalidArgument(
                "unable to derive available output filename".to_string(),
            ));
        }
        index += 1;
    }
}

fn parse_suffix_index(file_stem: &str) -> (&str, u64) {
    let Some((prefix, suffix)) = file_stem.rsplit_once('.') else {
        return (file_stem, 1);
    };

    if prefix.is_empty()
        || suffix.is_empty()
        || !suffix.chars().all(|character| character.is_ascii_digit())
    {
        return (file_stem, 1);
    }

    let Some(parsed_suffix) = suffix.parse::<u64>().ok() else {
        return (file_stem, 1);
    };

    if parsed_suffix == u64::MAX {
        return (prefix, u64::MAX);
    }

    (prefix, parsed_suffix + 1)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{parse_suffix_index, resolve_available_output_path};

    #[test]
    fn resolve_available_output_path_keeps_missing_path() {
        let dir = tempdir().expect("tempdir");
        let missing = dir.path().join("missing.txt");

        let output = resolve_available_output_path(&missing).expect("resolve available path");
        assert_eq!(output, missing);
    }

    #[test]
    fn resolve_available_output_path_adds_incremented_suffix() {
        let dir = tempdir().expect("tempdir");
        let target = dir.path().join("out.txt");
        fs::write(&target, b"already there").expect("write file");

        let output = resolve_available_output_path(&target).expect("resolve available path");
        assert_eq!(output, dir.path().join("out.1.txt"));
    }

    #[test]
    fn resolve_available_output_path_skips_existing_incremented_suffixes() {
        let dir = tempdir().expect("tempdir");
        let target = dir.path().join("out.txt");
        fs::write(&target, b"already there").expect("write file");
        fs::write(dir.path().join("out.1.txt"), b"already there").expect("write file");

        let output = resolve_available_output_path(&target).expect("resolve available path");
        assert_eq!(output, dir.path().join("out.2.txt"));
    }

    #[test]
    fn resolve_available_output_path_increments_existing_suffix() {
        let dir = tempdir().expect("tempdir");
        let target = dir.path().join("out.1.txt");
        fs::write(&target, b"already there").expect("write file");
        fs::write(dir.path().join("out.2.txt"), b"already there").expect("write file");

        let output = resolve_available_output_path(&target).expect("resolve available path");
        assert_eq!(output, dir.path().join("out.3.txt"));
    }

    #[test]
    fn resolve_available_output_path_handles_names_without_extension() {
        let dir = tempdir().expect("tempdir");
        let target = dir.path().join("out");
        fs::write(&target, b"already there").expect("write file");

        let output = resolve_available_output_path(&target).expect("resolve available path");
        assert_eq!(output, dir.path().join("out.1"));
    }

    #[test]
    fn parse_suffix_index_extracts_existing_counter() {
        assert_eq!(parse_suffix_index("note"), ("note", 1));
        assert_eq!(parse_suffix_index("note.2"), ("note", 3));
        assert_eq!(parse_suffix_index("archive.tar"), ("archive.tar", 1));
    }

    #[test]
    fn parse_suffix_index_ignores_invalid_counters() {
        assert_eq!(parse_suffix_index("note."), ("note.", 1));
        assert_eq!(parse_suffix_index(".note"), (".note", 1));
        assert_eq!(parse_suffix_index("note.a"), ("note.a", 1));
    }

    #[test]
    fn parse_suffix_index_handles_max_counter() {
        let stem = format!("note.{}", u64::MAX);
        let (base, start) = parse_suffix_index(&stem);

        assert_eq!(base, "note");
        assert_eq!(start, u64::MAX);
    }
}
