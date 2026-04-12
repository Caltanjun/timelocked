//! Orchestrates the parsing and resolution of user intent (duration vs explicit iterations)
//! to produce a concrete number of operations for a timelock puzzle.

use crate::base::{Error, Result};

use super::{
    estimate_duration_on_current_machine_seconds, estimate_duration_seconds_for_rate,
    estimate_iterations_for_rate, get_profile, parse_duration_spec, resolve_iterations_per_second,
    CURRENT_MACHINE_PROFILE_ID,
};

pub const DEFAULT_HARDWARE_PROFILE_ID: &str = "high-end-2026";

#[derive(Debug, Clone)]
pub struct ResolvedLockDifficulty {
    pub iterations: u64,
    pub hardware_profile_id: String,
    pub target_seconds: Option<u64>,
}

pub fn resolve_lock_difficulty(
    explicit_iterations: Option<u64>,
    target: Option<&str>,
    profile_id: Option<&str>,
    current_machine_iterations_per_second: Option<u64>,
) -> Result<ResolvedLockDifficulty> {
    let profile_id = profile_id.unwrap_or(DEFAULT_HARDWARE_PROFILE_ID);

    if let Some(iterations) = explicit_iterations {
        if iterations == 0 {
            return Err(Error::InvalidArgument(
                "iterations must be greater than zero".to_string(),
            ));
        }

        if profile_id != CURRENT_MACHINE_PROFILE_ID {
            get_profile(profile_id)?;
        }

        return Ok(ResolvedLockDifficulty {
            iterations,
            hardware_profile_id: profile_id.to_string(),
            target_seconds: None,
        });
    }

    let target = target.ok_or_else(|| {
        Error::InvalidArgument("either --target or --iterations must be provided".to_string())
    })?;

    let duration = parse_duration_spec(target)?;
    let rate = resolve_iterations_per_second(profile_id, current_machine_iterations_per_second)?;
    let iterations = estimate_iterations_for_rate(duration, rate)?;

    Ok(ResolvedLockDifficulty {
        iterations,
        hardware_profile_id: profile_id.to_string(),
        target_seconds: Some(duration.as_secs()),
    })
}

pub fn estimate_duration_on_profile_seconds(iterations: u64, profile_id: &str) -> Option<u64> {
    get_profile(profile_id)
        .ok()
        .map(|profile| iterations / profile.iterations_per_second.max(1))
}

pub fn estimate_duration_on_rate_seconds(
    iterations: u64,
    iterations_per_second: u64,
) -> Option<u64> {
    estimate_duration_seconds_for_rate(iterations, iterations_per_second)
}

pub fn estimate_duration_on_profile_choice_seconds(
    iterations: u64,
    profile_id: &str,
    current_machine_iterations_per_second: Option<u64>,
) -> Option<u64> {
    if profile_id == CURRENT_MACHINE_PROFILE_ID {
        return estimate_duration_on_current_machine_seconds(
            iterations,
            current_machine_iterations_per_second,
        );
    }

    estimate_duration_on_profile_seconds(iterations, profile_id)
}

#[cfg(test)]
mod tests {
    use crate::base::Error;
    use crate::domains::timelock::get_profile;

    use super::{
        estimate_duration_on_current_machine_seconds, estimate_duration_on_profile_choice_seconds,
        estimate_duration_on_profile_seconds, resolve_lock_difficulty,
    };

    #[test]
    fn resolve_lock_difficulty_uses_explicit_iterations() {
        let resolved =
            resolve_lock_difficulty(Some(42), None, Some("laptop-2024"), None).expect("resolved");

        assert_eq!(resolved.iterations, 42);
        assert_eq!(resolved.hardware_profile_id, "laptop-2024");
        assert_eq!(resolved.target_seconds, None);
    }

    #[test]
    fn resolve_lock_difficulty_treats_current_machine_as_metadata_for_explicit_iterations() {
        let resolved = resolve_lock_difficulty(Some(42), None, Some("current-machine"), None)
            .expect("resolved");

        assert_eq!(resolved.iterations, 42);
        assert_eq!(resolved.hardware_profile_id, "current-machine");
        assert_eq!(resolved.target_seconds, None);
    }

    #[test]
    fn resolve_lock_difficulty_rejects_invalid_inputs() {
        let err = resolve_lock_difficulty(Some(0), None, None, None).expect_err("must fail");
        assert!(matches!(err, Error::InvalidArgument(_)));

        let err = resolve_lock_difficulty(None, None, None, None).expect_err("must fail");
        assert!(matches!(err, Error::InvalidArgument(_)));
        assert!(err.to_string().contains("either --target or --iterations"));
    }

    #[test]
    fn resolve_lock_difficulty_computes_from_target_and_profile() {
        let profile = get_profile("desktop-2026").expect("known profile");
        let resolved = resolve_lock_difficulty(None, Some("2s"), Some("desktop-2026"), None)
            .expect("resolved");

        assert_eq!(resolved.iterations, 2 * profile.iterations_per_second);
        assert_eq!(resolved.hardware_profile_id, "desktop-2026");
        assert_eq!(resolved.target_seconds, Some(2));
    }

    #[test]
    fn resolve_lock_difficulty_accepts_current_machine_profile() {
        let resolved =
            resolve_lock_difficulty(None, Some("2s"), Some("current-machine"), Some(321))
                .expect("resolved");

        assert_eq!(resolved.iterations, 642);
        assert_eq!(resolved.hardware_profile_id, "current-machine");
    }

    #[test]
    fn estimate_duration_on_profile_seconds_returns_expected_values() {
        let profile = get_profile("desktop-2026").expect("known profile");
        assert_eq!(
            estimate_duration_on_profile_seconds(2 * profile.iterations_per_second, "desktop-2026"),
            Some(2)
        );
        assert_eq!(
            estimate_duration_on_profile_seconds(1, "unknown-profile"),
            None
        );
    }

    #[test]
    fn estimate_duration_on_profile_choice_supports_current_machine() {
        assert_eq!(
            estimate_duration_on_current_machine_seconds(642, Some(321)),
            Some(2)
        );
        assert_eq!(
            estimate_duration_on_profile_choice_seconds(642, "current-machine", Some(321)),
            Some(2)
        );
    }
}
