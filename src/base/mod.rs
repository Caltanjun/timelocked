//! Base primitives offering generic, reusable technical pieces with no Timelocked business concept.
//! Examples: Result types, cancellation tokens, error variations, progress models.

pub mod cancellation_token;
pub mod error;
pub mod file_manager;
pub mod hidden_entry;
pub mod path;
pub mod progress_status;
pub mod result;

pub use cancellation_token::{ensure_not_cancelled, CancellationToken};
pub use error::Error;
pub use file_manager::open_directory_in_file_manager;
pub use hidden_entry::is_hidden_entry_name;
pub use path::append_suffix_to_path;
pub use result::Result;
