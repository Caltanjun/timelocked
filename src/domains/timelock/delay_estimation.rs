//! Helpers for parsing and converting human-readable duration strings (e.g., "3d", "2h")
//! into cryptographic iteration counts based on a hardware profile.

use std::time::Duration;

use crate::base::{Error, Result};

use super::HardwareProfile;

pub fn parse_duration_spec(spec: &str) -> Result<Duration> {
    if spec.len() < 2 {
        return Err(Error::InvalidArgument(format!(
            "invalid duration '{spec}', expected formats like 6h, 3d, 2w"
        )));
    }

    let (amount_str, unit) = spec.split_at(spec.len() - 1);
    let amount: u64 = amount_str.parse().map_err(|_| {
        Error::InvalidArgument(format!(
            "invalid duration '{spec}', numeric value is not valid"
        ))
    })?;

    let seconds_per_unit = match unit {
        "s" => 1_u64,
        "m" => 60_u64,
        "h" => 3_600_u64,
        "d" => 86_400_u64,
        "w" => 604_800_u64,
        _ => {
            return Err(Error::InvalidArgument(format!(
                "invalid duration unit '{unit}', expected one of s, m, h, d, w"
            )))
        }
    };

    let secs = amount
        .checked_mul(seconds_per_unit)
        .ok_or_else(|| Error::InvalidArgument("duration is too large".to_string()))?;

    Ok(Duration::from_secs(secs))
}

pub fn estimate_iterations(duration: Duration, profile: HardwareProfile) -> Result<u64> {
    estimate_iterations_for_rate(duration, profile.iterations_per_second)
}

pub fn estimate_iterations_for_rate(duration: Duration, iterations_per_second: u64) -> Result<u64> {
    let secs = duration.as_secs() as u128;
    let rate = iterations_per_second as u128;
    let total = secs
        .checked_mul(rate)
        .ok_or_else(|| Error::InvalidArgument("iterations overflow".to_string()))?;

    if total == 0 {
        return Err(Error::InvalidArgument(
            "iterations cannot be zero".to_string(),
        ));
    }

    u64::try_from(total).map_err(|_| Error::InvalidArgument("iterations overflow u64".to_string()))
}

pub fn estimate_duration_seconds_for_rate(
    iterations: u64,
    iterations_per_second: u64,
) -> Option<u64> {
    if iterations_per_second == 0 {
        return None;
    }

    Some(iterations / iterations_per_second)
}

#[cfg(test)]
mod tests {
    use crate::domains::timelock::get_profile;

    use super::{
        estimate_duration_seconds_for_rate, estimate_iterations, estimate_iterations_for_rate,
        parse_duration_spec,
    };

    #[test]
    fn parses_duration_spec() {
        let duration = parse_duration_spec("3d").expect("must parse duration");
        assert_eq!(duration.as_secs(), 3 * 24 * 3600);
    }

    #[test]
    fn rejects_invalid_duration() {
        assert!(parse_duration_spec("3x").is_err());
        assert!(parse_duration_spec("h").is_err());
    }

    #[test]
    fn computes_iterations_from_profile() {
        let profile = get_profile("desktop-2026").expect("known profile");
        let duration = parse_duration_spec("2h").expect("duration");
        let iterations = estimate_iterations(duration, profile).expect("iterations");
        assert_eq!(iterations, 2 * 3600 * profile.iterations_per_second);
    }

    #[test]
    fn computes_iterations_from_explicit_rate() {
        let duration = parse_duration_spec("2s").expect("duration");
        let iterations = estimate_iterations_for_rate(duration, 123).expect("iterations");
        assert_eq!(iterations, 246);
    }

    #[test]
    fn computes_duration_from_explicit_rate() {
        assert_eq!(estimate_duration_seconds_for_rate(246, 123), Some(2));
        assert_eq!(estimate_duration_seconds_for_rate(1, 0), None);
    }
}
