//! Benchmarks repeated squaring on the current machine and exposes helpers for
//! session-scoped machine-specific timelock estimates.

use std::time::Duration;

use crate::base::{Error, Result};

use super::{
    benchmark_repeated_squaring_iterations_per_second, estimate_duration_seconds_for_rate,
    get_profile, HardwareProfile,
};

pub const CURRENT_MACHINE_PROFILE_ID: &str = "current-machine";
const CALIBRATION_WINDOW: Duration = Duration::from_millis(750);

pub fn calibrate_current_machine_iterations_per_second() -> Result<u64> {
    let rate = benchmark_repeated_squaring_iterations_per_second(CALIBRATION_WINDOW);

    if rate == 0 {
        return Err(Error::InvalidArgument(
            "machine calibration produced zero iterations".to_string(),
        ));
    }

    Ok(rate)
}

pub fn is_current_machine_profile_id(profile_id: &str) -> bool {
    profile_id == CURRENT_MACHINE_PROFILE_ID
}

pub fn resolve_profile_or_current_machine(
    profile_id: &str,
    current_machine_iterations_per_second: Option<u64>,
) -> Result<Option<HardwareProfile>> {
    if is_current_machine_profile_id(profile_id) {
        ensure_current_machine_rate(current_machine_iterations_per_second)?;
        return Ok(None);
    }

    get_profile(profile_id).map(Some)
}

pub fn resolve_iterations_per_second(
    profile_id: &str,
    current_machine_iterations_per_second: Option<u64>,
) -> Result<u64> {
    if is_current_machine_profile_id(profile_id) {
        return ensure_current_machine_rate(current_machine_iterations_per_second);
    }

    Ok(get_profile(profile_id)?.iterations_per_second)
}

pub fn estimate_duration_on_current_machine_seconds(
    iterations: u64,
    current_machine_iterations_per_second: Option<u64>,
) -> Option<u64> {
    estimate_duration_seconds_for_rate(iterations, current_machine_iterations_per_second?)
}

fn ensure_current_machine_rate(current_machine_iterations_per_second: Option<u64>) -> Result<u64> {
    match current_machine_iterations_per_second {
        Some(0) => Err(Error::InvalidArgument(
            "current-machine calibration must be greater than zero".to_string(),
        )),
        Some(rate) => Ok(rate),
        None => Err(Error::InvalidArgument(
            "current-machine hardware profile requires a machine calibration".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        calibrate_current_machine_iterations_per_second,
        estimate_duration_on_current_machine_seconds, is_current_machine_profile_id,
        resolve_iterations_per_second, CURRENT_MACHINE_PROFILE_ID,
    };

    #[test]
    fn calibrates_to_positive_rate() {
        let rate = calibrate_current_machine_iterations_per_second().expect("rate");
        assert!(rate > 0);
    }

    #[test]
    fn resolves_current_machine_rate() {
        let rate = resolve_iterations_per_second(CURRENT_MACHINE_PROFILE_ID, Some(321))
            .expect("current machine rate");
        assert_eq!(rate, 321);
        assert!(is_current_machine_profile_id(CURRENT_MACHINE_PROFILE_ID));
    }

    #[test]
    fn estimates_duration_on_current_machine() {
        assert_eq!(
            estimate_duration_on_current_machine_seconds(642, Some(321)),
            Some(2)
        );
        assert_eq!(estimate_duration_on_current_machine_seconds(1, None), None);
    }
}
