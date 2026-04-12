//! Runs the current-machine calibration flow and returns the measured repeated
//! squaring rate for the active session.

use crate::base::Result;
use crate::domains::timelock::calibrate_current_machine_iterations_per_second;

#[derive(Debug, Clone)]
pub struct CalibrateResponse {
    pub iterations_per_second: u64,
}

pub fn execute() -> Result<CalibrateResponse> {
    Ok(CalibrateResponse {
        iterations_per_second: calibrate_current_machine_iterations_per_second()?,
    })
}

#[cfg(test)]
mod tests {
    use super::execute;

    #[test]
    fn calibrate_returns_positive_rate() {
        let response = execute().expect("calibration response");
        assert!(response.iterations_per_second > 0);
    }
}
