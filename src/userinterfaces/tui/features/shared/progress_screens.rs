//! Shared constructors for worker-backed progress screens in the TUI.

use crate::base::progress_status::ProgressStatus;
use crate::userinterfaces::tui::app_state::Screen;
use crate::userinterfaces::tui::features::lock::progress::{LockProgressFocus, LockProgressState};
use crate::userinterfaces::tui::features::unlock::progress::{
    UnlockProgressFocus, UnlockProgressState,
};
use crate::userinterfaces::tui::worker::{LockWorker, UnlockWorker};

pub(crate) fn new_lock_progress_screen(
    input_display: String,
    output_display: String,
    worker: LockWorker,
) -> Screen {
    Screen::LockProgress(LockProgressState {
        input_display,
        output_display,
        progress: ProgressStatus::new("starting", 0, 1, None, None),
        worker,
        cancel_requested: false,
        focus: LockProgressFocus::Progress,
    })
}

pub(crate) fn new_unlock_progress_screen(
    file_display: String,
    estimated_duration_seconds: Option<u64>,
    worker: UnlockWorker,
) -> Screen {
    Screen::UnlockProgress(new_unlock_like_progress_state(
        file_display,
        estimated_duration_seconds,
        worker,
    ))
}

fn new_unlock_like_progress_state(
    file_display: String,
    estimated_duration_seconds: Option<u64>,
    worker: UnlockWorker,
) -> UnlockProgressState {
    UnlockProgressState {
        file_display,
        progress: ProgressStatus::new("unlock-timelock", 0, 1, estimated_duration_seconds, None),
        worker,
        cancel_requested: false,
        cpu_count: available_cpu_count(),
        focus: UnlockProgressFocus::Progress,
    }
}

fn available_cpu_count() -> usize {
    std::thread::available_parallelism()
        .map(|parallelism| parallelism.get())
        .unwrap_or(1)
}
