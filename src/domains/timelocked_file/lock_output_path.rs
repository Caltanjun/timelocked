//! Domain-neutral helpers for `.timelocked` output paths.
//! Handles explicit output normalization and default sibling-path derivation.

use std::path::{Path, PathBuf};

use crate::base::append_suffix_to_path;

const TIMELOCKED_EXTENSION: &str = ".timelocked";

pub fn default_timelocked_output_path(input_path: &Path) -> PathBuf {
    append_suffix_to_path(input_path, TIMELOCKED_EXTENSION)
}

pub fn ensure_timelocked_extension(path: PathBuf) -> PathBuf {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("timelocked"))
    {
        return path;
    }

    default_timelocked_output_path(&path)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{default_timelocked_output_path, ensure_timelocked_extension};

    #[test]
    fn default_timelocked_output_path_appends_suffix_for_file_inputs() {
        let resolved = default_timelocked_output_path(Path::new("message.txt"));
        assert_eq!(resolved, PathBuf::from("message.txt.timelocked"));
    }

    #[test]
    fn ensure_timelocked_extension_keeps_existing_extension() {
        let explicit = PathBuf::from("message.timelocked");

        let resolved = ensure_timelocked_extension(explicit.clone());
        assert_eq!(resolved, explicit);
    }

    #[test]
    fn ensure_timelocked_extension_appends_extension_without_existing_extension() {
        let explicit = PathBuf::from("message");

        let resolved = ensure_timelocked_extension(explicit);
        assert_eq!(resolved, PathBuf::from("message.timelocked"));
    }

    #[test]
    fn ensure_timelocked_extension_appends_after_other_extension() {
        let explicit = PathBuf::from("archive.bin");

        let resolved = ensure_timelocked_extension(explicit);
        assert_eq!(resolved, PathBuf::from("archive.bin.timelocked"));
    }
}
