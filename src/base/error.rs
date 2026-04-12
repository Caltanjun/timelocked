//! Global domain and application errors for the Timelocked project.
//! Centralizes error variants to map accurately across the tool (e.g., IO, Crypto, Format).

use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    #[error("invalid format: {0}")]
    InvalidFormat(String),

    #[error("unsupported format version: {0}")]
    UnsupportedVersion(u8),

    #[error("output already exists: {0}")]
    OutputExists(PathBuf),

    #[error("cryptographic error: {0}")]
    Crypto(String),

    #[error("verification failed: {0}")]
    Verification(String),

    #[error("operation cancelled")]
    Cancelled,
}
