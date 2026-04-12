//! Shared unlock ETA and session-calibration helpers for TUI flows.

use std::path::PathBuf;

use crate::userinterfaces::tui::app_state::App;
use crate::userinterfaces::tui::features::shared::timelock_estimate::inspect_timelock_estimate;
use crate::userinterfaces::tui::features::unlock::form::UnlockFormState;

pub(crate) fn refresh_unlock_estimate_state(app: &mut App, state: &mut UnlockFormState) {
    state.estimated_duration_label = None;
    state.estimated_duration_seconds = None;
    state.estimated_error = None;

    let input = state.input_path.value.trim();
    if input.is_empty() {
        return;
    }

    let input_path = PathBuf::from(input);
    if !input_path.exists() {
        state.estimated_error = Some("File not found".to_string());
        return;
    }

    match inspect_timelock_estimate(app, &input_path) {
        Ok(Some(estimate)) => {
            state.estimated_duration_seconds = Some(estimate.seconds);
            state.estimated_duration_label = Some(estimate.label);
        }
        Ok(None) => {}
        Err(err) => {
            state.estimated_error = Some(err);
        }
    }
}
