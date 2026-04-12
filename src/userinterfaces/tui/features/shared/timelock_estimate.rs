//! Shared inspect-backed ETA helpers for TUI flows that may run a timelock.

use std::path::Path;

use crate::usecases::inspect;
use crate::userinterfaces::common::output::format_eta;
use crate::userinterfaces::tui::app_state::App;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TimelockEstimate {
    pub label: String,
    pub seconds: u64,
}

pub(crate) fn inspect_timelock_estimate(
    app: &mut App,
    input_path: &Path,
) -> Result<Option<TimelockEstimate>, String> {
    let response = inspect::execute(inspect::InspectRequest {
        input: input_path.to_path_buf(),
        current_machine_iterations_per_second: app.estimate_calibration_for_path(input_path),
    })
    .map_err(|err| err.to_string())?;

    if let Some(seconds) = response.estimated_duration_on_current_machine_seconds {
        return Ok(Some(TimelockEstimate {
            label: format!("Estimated time on this machine: {}", format_eta(seconds)),
            seconds,
        }));
    }

    if let Some(seconds) = response.estimated_duration_on_profile_seconds {
        return Ok(Some(TimelockEstimate {
            label: format!(
                "Estimated time from chosen profile: {}",
                format_eta(seconds)
            ),
            seconds,
        }));
    }

    Ok(None)
}
