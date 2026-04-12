//! Models the progress state of operations to be displayed by UIs.
//! Encapsulates completion percentages, rates, and ETAs.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ProgressStatus {
    pub phase: String,
    pub current: u64,
    pub total: u64,
    pub pct: f64,
    pub eta_seconds: Option<u64>,
    pub rate_per_second: Option<f64>,
}

impl ProgressStatus {
    pub fn new(
        phase: impl Into<String>,
        current: u64,
        total: u64,
        eta_seconds: Option<u64>,
        rate_per_second: Option<f64>,
    ) -> Self {
        let pct = if total == 0 {
            0.0
        } else {
            (current as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
        };

        Self {
            phase: phase.into(),
            current,
            total,
            pct,
            eta_seconds,
            rate_per_second,
        }
    }
}
