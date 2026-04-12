//! Models hardware performance profiles used to estimate how many iterations
//! of repeated squaring correspond to a wall-clock delay on average hardware.

use serde::Serialize;

use crate::base::{Error, Result};

#[derive(Debug, Clone, Copy, Serialize)]
pub struct HardwareProfile {
    pub id: &'static str,
    pub label: &'static str,
    pub iterations_per_second: u64,
}

const PROFILES: [HardwareProfile; 3] = [
    HardwareProfile {
        id: "laptop-2024",
        label: "Laptop CPU (2024)",
        iterations_per_second: 250_000,
    },
    HardwareProfile {
        id: "desktop-2026",
        label: "Desktop CPU (2026)",
        iterations_per_second: 400_000,
    },
    HardwareProfile {
        id: "high-end-2026",
        label: "High-end CPU (2026)",
        iterations_per_second: 500_000,
    },
];

pub fn all_profiles() -> &'static [HardwareProfile] {
    &PROFILES
}

pub fn get_profile(profile_id: &str) -> Result<HardwareProfile> {
    PROFILES
        .iter()
        .copied()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| {
            let known = PROFILES
                .iter()
                .map(|profile| profile.id)
                .collect::<Vec<_>>()
                .join(", ");
            Error::InvalidArgument(format!(
                "unknown hardware profile '{profile_id}', expected one of: {known}"
            ))
        })
}
