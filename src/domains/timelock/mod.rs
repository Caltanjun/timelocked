//! Timelock domain module exposing algorithms and business rules for elapsed-time cryptography.
//! Contains difficulty calculation, hardware profiles, and puzzle generation.

mod calibration_service;
mod delay_estimation;
mod hardware_profile;
mod lock_difficulty;
mod puzzle_material;
mod puzzle_service;

pub use calibration_service::{
    calibrate_current_machine_iterations_per_second, estimate_duration_on_current_machine_seconds,
    is_current_machine_profile_id, resolve_iterations_per_second,
    resolve_profile_or_current_machine, CURRENT_MACHINE_PROFILE_ID,
};
pub use delay_estimation::{
    estimate_duration_seconds_for_rate, estimate_iterations, estimate_iterations_for_rate,
    parse_duration_spec,
};
pub use hardware_profile::{all_profiles, get_profile, HardwareProfile};
pub use lock_difficulty::{
    estimate_duration_on_profile_choice_seconds, estimate_duration_on_profile_seconds,
    estimate_duration_on_rate_seconds, resolve_lock_difficulty, ResolvedLockDifficulty,
    DEFAULT_HARDWARE_PROFILE_ID,
};
pub use puzzle_material::{TimelockPuzzleMaterial, FILE_KEY_SIZE};
pub use puzzle_service::{
    benchmark_repeated_squaring_iterations, benchmark_repeated_squaring_iterations_per_second,
    create_puzzle_and_wrap_key, unwrap_key, unwrap_key_with_cancel,
};
