//! Resolves current-machine calibration only when the lock request requires it.

use crate::base::Result;
use crate::domains::timelock::{
    calibrate_current_machine_iterations_per_second, is_current_machine_profile_id,
};

use super::LockRequest;

pub(super) fn resolve_current_machine_iterations_per_second(
    request: &LockRequest,
) -> Result<Option<u64>> {
    if request.iterations.is_some() {
        return Ok(None);
    }

    let Some(profile_id) = request.hardware_profile.as_deref() else {
        return Ok(None);
    };

    if !is_current_machine_profile_id(profile_id) {
        return Ok(None);
    }

    match request.current_machine_iterations_per_second {
        Some(rate) => Ok(Some(rate)),
        None => Ok(Some(calibrate_current_machine_iterations_per_second()?)),
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_current_machine_iterations_per_second;
    use crate::usecases::lock::LockRequest;

    #[test]
    fn resolves_current_machine_rate_only_when_requested() {
        let ordinary = LockRequest {
            input: "hello".to_string(),
            output: None,
            modulus_bits: 256,
            target: Some("1s".to_string()),
            iterations: None,
            hardware_profile: Some("desktop-2026".to_string()),
            current_machine_iterations_per_second: Some(77),
            creator_name: None,
            creator_message: None,
            verify: false,
        };
        assert_eq!(
            resolve_current_machine_iterations_per_second(&ordinary).expect("ordinary rate"),
            None
        );

        let current_machine_target = LockRequest {
            hardware_profile: Some("current-machine".to_string()),
            current_machine_iterations_per_second: Some(77),
            ..ordinary.clone()
        };
        assert_eq!(
            resolve_current_machine_iterations_per_second(&current_machine_target)
                .expect("current machine rate"),
            Some(77)
        );

        let current_machine_iterations = LockRequest {
            iterations: Some(42),
            target: None,
            hardware_profile: Some("current-machine".to_string()),
            current_machine_iterations_per_second: Some(77),
            ..ordinary
        };
        assert_eq!(
            resolve_current_machine_iterations_per_second(&current_machine_iterations)
                .expect("explicit iterations skip calibration"),
            None
        );
    }
}
