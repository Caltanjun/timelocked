//! Cross-platform file-manager launching helpers.
//! Keeps OS-specific command selection isolated from UI features.

use std::path::Path;
use std::process::Command;

use crate::base::{Error, Result};

pub fn open_directory_in_file_manager(path: &Path) -> Result<()> {
    let status = file_manager_command(path).status().map_err(|err| {
        Error::Io(std::io::Error::new(
            err.kind(),
            format!("failed to open folder: {err}"),
        ))
    })?;

    if status.success() {
        Ok(())
    } else {
        Err(Error::InvalidArgument(
            "file manager command exited with failure status".to_string(),
        ))
    }
}

#[cfg(target_os = "macos")]
fn file_manager_command(path: &Path) -> Command {
    let mut command = Command::new("open");
    command.arg(path);
    command
}

#[cfg(target_os = "windows")]
fn file_manager_command(path: &Path) -> Command {
    let mut command = Command::new("explorer");
    command.arg(path);
    command
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn file_manager_command(path: &Path) -> Command {
    let mut command = Command::new("xdg-open");
    command.arg(path);
    command
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::file_manager_command;

    #[cfg(target_os = "macos")]
    #[test]
    fn file_manager_command_uses_open_on_macos() {
        let command = file_manager_command(Path::new("folder"));
        assert_eq!(command.get_program().to_string_lossy(), "open");
        assert_eq!(
            command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["folder".to_string()]
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn file_manager_command_uses_explorer_on_windows() {
        let command = file_manager_command(Path::new("folder"));
        assert_eq!(command.get_program().to_string_lossy(), "explorer");
        assert_eq!(
            command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["folder".to_string()]
        );
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    #[test]
    fn file_manager_command_uses_xdg_open_on_other_targets() {
        let command = file_manager_command(Path::new("folder"));
        assert_eq!(command.get_program().to_string_lossy(), "xdg-open");
        assert_eq!(
            command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["folder".to_string()]
        );
    }
}
