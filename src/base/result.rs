//! Core Result type alias, standardizing returns to Timelocked's generic `Error` enum.

use super::error::Error;

pub type Result<T> = std::result::Result<T, Error>;
