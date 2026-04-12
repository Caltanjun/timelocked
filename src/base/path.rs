//! Generic path helpers for filename-preserving suffix operations.
//! Keeps filesystem path manipulation reusable and free of Timelocked concepts.

use std::path::{Path, PathBuf};

pub fn append_suffix_to_path(path: &Path, suffix: &str) -> PathBuf {
    match path.file_name() {
        Some(filename) => {
            let mut filename_with_suffix = filename.to_os_string();
            filename_with_suffix.push(suffix);
            path.with_file_name(filename_with_suffix)
        }
        None => {
            let mut path_with_suffix = path.as_os_str().to_os_string();
            path_with_suffix.push(suffix);
            PathBuf::from(path_with_suffix)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::append_suffix_to_path;

    #[test]
    fn append_suffix_to_path_appends_to_filename() {
        let resolved = append_suffix_to_path(Path::new("notes.txt"), ".timelocked");
        assert_eq!(resolved, PathBuf::from("notes.txt.timelocked"));
    }

    #[test]
    fn append_suffix_to_path_preserves_parent_directories() {
        let resolved = append_suffix_to_path(Path::new("nested/notes.txt"), ".bak");
        assert_eq!(resolved, PathBuf::from("nested/notes.txt.bak"));
    }
}
