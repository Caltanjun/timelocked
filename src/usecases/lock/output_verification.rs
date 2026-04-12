//! Runs optional post-write verification for the final `.timelocked` artifact.

use std::path::Path;

use crate::base::{CancellationToken, Result};
use crate::domains::timelocked_file::{
    parse_container, verify_timelocked_file_structural_and_cancel,
};

pub(super) fn verify_output_if_requested(
    verify: bool,
    output_path: &Path,
    cancellation: Option<&CancellationToken>,
) -> Result<()> {
    if !verify {
        return Ok(());
    }

    let parsed = parse_container(output_path)?;
    verify_timelocked_file_structural_and_cancel(output_path, &parsed, cancellation).map(|_| ())
}
