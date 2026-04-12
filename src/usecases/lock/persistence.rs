//! Persists the final `.timelocked` artifact from a staged temporary file.

use std::path::Path;

use crate::base::{Error, Result};

pub(super) fn persist_timelocked_container(
    output_path: &Path,
    artifact_temp: tempfile::NamedTempFile,
) -> Result<()> {
    artifact_temp
        .persist(output_path)
        .map_err(|err| Error::Io(err.error))?;
    Ok(())
}
