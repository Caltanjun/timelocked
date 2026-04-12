//! TUI file-browser state and directory listing helpers.
//! Keeps browsing logic local while delegating platform-sensitive entry rules to `base`.

use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

use crate::base::is_hidden_entry_name;

#[derive(Debug, Clone, Copy)]
pub enum BrowserMode {
    File,
    Directory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserFileFilter {
    TimelockedOnly,
    AllFiles,
}

#[derive(Debug, Clone, Copy)]
pub enum BrowserTarget {
    LockFileInput,
    UnlockInput,
    UnlockOutputDir,
    InspectInput,
    VerifyInput,
}

#[derive(Debug, Clone)]
pub struct BrowserEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
}

#[derive(Debug, Clone)]
pub struct FileBrowserState {
    pub mode: BrowserMode,
    pub target: BrowserTarget,
    pub file_filter: BrowserFileFilter,
    pub show_hidden: bool,
    pub current_dir: PathBuf,
    pub entries: Vec<BrowserEntry>,
    pub selected: usize,
    pub error: Option<String>,
}

impl FileBrowserState {
    pub(crate) fn new(
        mode: BrowserMode,
        target: BrowserTarget,
        preferred_path: Option<PathBuf>,
    ) -> Self {
        let current_dir = preferred_path
            .and_then(resolve_browse_start_dir)
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        let file_filter = default_browser_file_filter(mode, target);

        let mut state = Self {
            mode,
            target,
            file_filter,
            show_hidden: false,
            current_dir,
            entries: Vec::new(),
            selected: 0,
            error: None,
        };
        state.reload_entries();
        state
    }

    pub(crate) fn toggle_file_filter(&mut self) {
        if !browser_filter_toggle_available(self.mode, self.target) {
            return;
        }

        self.file_filter = match self.file_filter {
            BrowserFileFilter::TimelockedOnly => BrowserFileFilter::AllFiles,
            BrowserFileFilter::AllFiles => BrowserFileFilter::TimelockedOnly,
        };
        self.reload_entries();
    }

    pub(crate) fn toggle_hidden_entries(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.reload_entries();
    }

    pub(crate) fn selected_entry(&self) -> Option<&BrowserEntry> {
        self.entries.get(self.selected)
    }

    pub(crate) fn move_up(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        if self.selected == 0 {
            self.selected = self.entries.len() - 1;
        } else {
            self.selected -= 1;
        }
    }

    pub(crate) fn move_down(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.entries.len();
    }

    pub(crate) fn navigate_selected(&mut self) {
        let Some(entry) = self.selected_entry() else {
            return;
        };
        if entry.is_dir {
            self.current_dir = entry.path.clone();
            self.reload_entries();
        }
    }

    pub(crate) fn navigate_parent(&mut self) {
        if let Some(parent) = self.current_dir.parent() {
            self.current_dir = parent.to_path_buf();
            self.reload_entries();
        }
    }

    fn reload_entries(&mut self) {
        self.error = None;
        match read_browser_entries(
            &self.current_dir,
            self.mode,
            self.file_filter,
            self.show_hidden,
        ) {
            Ok(entries) => {
                self.selected =
                    default_browser_selection_index(&entries, self.mode, self.file_filter);
                self.entries = entries;
            }
            Err(err) => {
                self.entries.clear();
                self.selected = 0;
                self.error = Some(err.to_string());
            }
        }
    }
}

fn resolve_browse_start_dir(path: PathBuf) -> Option<PathBuf> {
    if path.as_os_str().is_empty() {
        return None;
    }
    if path.is_dir() {
        return Some(path);
    }
    if path.is_file() {
        return path.parent().map(Path::to_path_buf);
    }
    path.parent().map(Path::to_path_buf)
}

fn default_browser_file_filter(mode: BrowserMode, target: BrowserTarget) -> BrowserFileFilter {
    if !matches!(mode, BrowserMode::File) {
        return BrowserFileFilter::AllFiles;
    }

    match target {
        BrowserTarget::LockFileInput | BrowserTarget::UnlockOutputDir => {
            BrowserFileFilter::AllFiles
        }
        BrowserTarget::UnlockInput | BrowserTarget::InspectInput | BrowserTarget::VerifyInput => {
            BrowserFileFilter::TimelockedOnly
        }
    }
}

pub(crate) fn browser_filter_toggle_available(mode: BrowserMode, target: BrowserTarget) -> bool {
    matches!(mode, BrowserMode::File)
        && matches!(
            target,
            BrowserTarget::UnlockInput | BrowserTarget::InspectInput | BrowserTarget::VerifyInput
        )
}

pub(crate) fn read_browser_entries(
    dir: &Path,
    mode: BrowserMode,
    file_filter: BrowserFileFilter,
    show_hidden: bool,
) -> std::io::Result<Vec<BrowserEntry>> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !show_hidden && is_hidden_entry_name(&name) {
            continue;
        }

        let path = entry.path();
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        let is_dir = metadata.is_dir();
        if matches!(mode, BrowserMode::Directory) && !is_dir {
            continue;
        }

        if !is_dir
            && matches!(mode, BrowserMode::File)
            && matches!(file_filter, BrowserFileFilter::TimelockedOnly)
            && !is_timelocked_file(&path)
        {
            continue;
        }

        entries.push(BrowserEntry { name, path, is_dir });
    }

    entries.sort_by(|left, right| match (left.is_dir, right.is_dir) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => left
            .name
            .to_ascii_lowercase()
            .cmp(&right.name.to_ascii_lowercase()),
    });
    Ok(entries)
}

fn default_browser_selection_index(
    entries: &[BrowserEntry],
    mode: BrowserMode,
    file_filter: BrowserFileFilter,
) -> usize {
    if entries.is_empty() {
        return 0;
    }

    if matches!(mode, BrowserMode::File) && matches!(file_filter, BrowserFileFilter::TimelockedOnly)
    {
        return entries.iter().position(|entry| !entry.is_dir).unwrap_or(0);
    }

    0
}

fn is_timelocked_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("timelocked"))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{
        read_browser_entries, BrowserFileFilter, BrowserMode, BrowserTarget, FileBrowserState,
    };

    #[test]
    fn defaults_to_timelocked_filter_for_unlock_input() {
        let dir = tempdir().expect("tempdir");
        let state = FileBrowserState::new(
            BrowserMode::File,
            BrowserTarget::UnlockInput,
            Some(dir.path().to_path_buf()),
        );

        assert!(matches!(
            state.file_filter,
            BrowserFileFilter::TimelockedOnly
        ));
    }

    #[test]
    fn defaults_to_all_files_for_lock_input() {
        let dir = tempdir().expect("tempdir");
        let state = FileBrowserState::new(
            BrowserMode::File,
            BrowserTarget::LockFileInput,
            Some(dir.path().to_path_buf()),
        );

        assert!(matches!(state.file_filter, BrowserFileFilter::AllFiles));
    }

    #[test]
    fn defaults_to_timelocked_filter_for_verify_input() {
        let dir = tempdir().expect("tempdir");
        let state = FileBrowserState::new(
            BrowserMode::File,
            BrowserTarget::VerifyInput,
            Some(dir.path().to_path_buf()),
        );

        assert!(matches!(
            state.file_filter,
            BrowserFileFilter::TimelockedOnly
        ));
    }

    #[test]
    fn read_entries_applies_timelocked_filter() {
        let dir = tempdir().expect("tempdir");
        fs::create_dir(dir.path().join("nested")).expect("create nested dir");
        fs::write(dir.path().join("one.timelocked"), b"x").expect("write timelocked file");
        fs::write(dir.path().join("two.txt"), b"x").expect("write non-timelocked file");

        let entries = read_browser_entries(
            dir.path(),
            BrowserMode::File,
            BrowserFileFilter::TimelockedOnly,
            false,
        )
        .expect("read entries");

        let names = entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>();
        assert!(names.contains(&"nested"));
        assert!(names.contains(&"one.timelocked"));
        assert!(!names.contains(&"two.txt"));
    }

    #[test]
    fn toggle_filter_switches_between_modes() {
        let dir = tempdir().expect("tempdir");
        let mut state = FileBrowserState::new(
            BrowserMode::File,
            BrowserTarget::InspectInput,
            Some(dir.path().to_path_buf()),
        );

        assert!(matches!(
            state.file_filter,
            BrowserFileFilter::TimelockedOnly
        ));
        state.toggle_file_filter();
        assert!(matches!(state.file_filter, BrowserFileFilter::AllFiles));
        state.toggle_file_filter();
        assert!(matches!(
            state.file_filter,
            BrowserFileFilter::TimelockedOnly
        ));
    }

    #[test]
    fn read_entries_hides_hidden_entries_by_default() {
        let dir = tempdir().expect("tempdir");
        fs::create_dir(dir.path().join(".hidden-dir")).expect("create hidden dir");
        fs::write(dir.path().join(".secret.timelocked"), b"x").expect("write hidden file");
        fs::write(dir.path().join("visible.timelocked"), b"x").expect("write visible file");

        let entries = read_browser_entries(
            dir.path(),
            BrowserMode::File,
            BrowserFileFilter::AllFiles,
            false,
        )
        .expect("read entries");

        let names = entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>();
        assert!(!names.contains(&".hidden-dir"));
        assert!(!names.contains(&".secret.timelocked"));
        assert!(names.contains(&"visible.timelocked"));
    }

    #[test]
    fn toggle_hidden_entries_reveals_hidden_files() {
        let dir = tempdir().expect("tempdir");
        fs::write(dir.path().join(".secret.timelocked"), b"x").expect("write hidden file");
        fs::write(dir.path().join("visible.timelocked"), b"x").expect("write visible file");

        let mut state = FileBrowserState::new(
            BrowserMode::File,
            BrowserTarget::InspectInput,
            Some(dir.path().to_path_buf()),
        );

        let before = state
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>();
        assert!(!before.contains(&".secret.timelocked"));
        assert!(before.contains(&"visible.timelocked"));

        state.toggle_hidden_entries();

        let after = state
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>();
        assert!(after.contains(&".secret.timelocked"));
        assert!(after.contains(&"visible.timelocked"));
    }

    #[test]
    fn does_not_toggle_filter_for_lock_input() {
        let dir = tempdir().expect("tempdir");
        let mut state = FileBrowserState::new(
            BrowserMode::File,
            BrowserTarget::LockFileInput,
            Some(dir.path().to_path_buf()),
        );

        assert!(matches!(state.file_filter, BrowserFileFilter::AllFiles));
        state.toggle_file_filter();
        assert!(matches!(state.file_filter, BrowserFileFilter::AllFiles));
    }

    #[test]
    fn prefers_first_timelocked_file_when_filter_is_active() {
        let dir = tempdir().expect("tempdir");
        fs::create_dir(dir.path().join("aaa")).expect("create aaa dir");
        fs::create_dir(dir.path().join("bbb")).expect("create bbb dir");
        fs::write(dir.path().join("one.timelocked"), b"x").expect("write timelocked file");

        let state = FileBrowserState::new(
            BrowserMode::File,
            BrowserTarget::UnlockInput,
            Some(dir.path().to_path_buf()),
        );

        assert!(state.selected_entry().is_some_and(|entry| !entry.is_dir));
        assert_eq!(
            state
                .selected_entry()
                .map(|entry| entry.name.as_str())
                .unwrap_or(""),
            "one.timelocked"
        );
    }
}
