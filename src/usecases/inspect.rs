//! Reads timelocked container metadata and computes delay estimates without
//! performing the expensive unlock work.

use std::path::PathBuf;

use crate::base::Result;
use crate::domains::timelock::{
    estimate_duration_on_current_machine_seconds, estimate_duration_on_profile_seconds,
};
use crate::domains::timelocked_file::{parse_container, TimelockedHeader, BODY_VERSION_V1};

#[derive(Debug, Clone)]
pub struct InspectRequest {
    pub input: PathBuf,
    pub current_machine_iterations_per_second: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct InspectResponse {
    pub path: PathBuf,
    pub payload_len: u64,
    pub format_version: u8,
    pub header: TimelockedHeader,
    pub estimated_duration_on_profile_seconds: Option<u64>,
    pub estimated_duration_on_current_machine_seconds: Option<u64>,
}

pub fn execute(request: InspectRequest) -> Result<InspectResponse> {
    let parsed = parse_container(&request.input)?;
    let estimated_duration_on_profile_seconds = estimate_duration_on_profile_seconds(
        parsed.superblock.iterations,
        &parsed.superblock.hardware_profile,
    );
    let estimated_duration_on_current_machine_seconds =
        estimate_duration_on_current_machine_seconds(
            parsed.superblock.iterations,
            request.current_machine_iterations_per_second,
        );

    Ok(InspectResponse {
        path: request.input,
        payload_len: parsed.superblock.payload_region_len,
        format_version: BODY_VERSION_V1,
        header: parsed.header,
        estimated_duration_on_profile_seconds,
        estimated_duration_on_current_machine_seconds,
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use crate::domains::timelock::get_profile;
    use crate::domains::timelocked_file::test_support::SampleTimelockedFileBuilder;

    use super::{execute, InspectRequest};

    #[test]
    fn inspect_estimates_duration_for_known_profile() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("known.timelocked");
        let profile = get_profile("desktop-2026").expect("known profile");

        SampleTimelockedFileBuilder::new(Vec::<u8>::new())
            .original_filename("note.txt")
            .iterations(2 * profile.iterations_per_second)
            .hardware_profile("desktop-2026")
            .target_seconds(Some(2))
            .write_to(&path)
            .expect("write artifact");

        let response = execute(InspectRequest {
            input: path.clone(),
            current_machine_iterations_per_second: None,
        })
        .expect("inspect response");

        assert_eq!(response.path, path);
        assert_eq!(response.format_version, 1);
        assert!(response.payload_len > 0);
        assert_eq!(response.estimated_duration_on_profile_seconds, Some(2));
        assert_eq!(response.estimated_duration_on_current_machine_seconds, None);
    }

    #[test]
    fn inspect_returns_none_for_unknown_profile() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("unknown.timelocked");

        SampleTimelockedFileBuilder::new(Vec::<u8>::new())
            .original_filename("note.txt")
            .iterations(10)
            .hardware_profile("unknown-profile")
            .write_to(&path)
            .expect("write artifact");

        let response = execute(InspectRequest {
            input: path,
            current_machine_iterations_per_second: None,
        })
        .expect("inspect response");
        assert_eq!(response.estimated_duration_on_profile_seconds, None);
    }

    #[test]
    fn inspect_estimates_duration_for_current_machine_when_available() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("machine.timelocked");

        SampleTimelockedFileBuilder::new(Vec::<u8>::new())
            .original_filename("note.txt")
            .iterations(642)
            .hardware_profile("current-machine")
            .target_seconds(Some(2))
            .write_to(&path)
            .expect("write artifact");

        let response = execute(InspectRequest {
            input: path,
            current_machine_iterations_per_second: Some(321),
        })
        .expect("inspect response");

        assert_eq!(response.estimated_duration_on_profile_seconds, None);
        assert_eq!(
            response.estimated_duration_on_current_machine_seconds,
            Some(2)
        );
    }
}
