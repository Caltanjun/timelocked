//! Base primitive for cooperative cancellation across threads.
//! Allows long-running tasks like crypto operations to be safely aborted.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::{Error, Result};

#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

pub fn ensure_not_cancelled(cancellation: Option<&CancellationToken>) -> Result<()> {
    if cancellation.is_some_and(|token| token.is_cancelled()) {
        return Err(Error::Cancelled);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ensure_not_cancelled, CancellationToken};
    use crate::base::Error;

    #[test]
    fn cancellation_token_is_shared_across_clones() {
        let original = CancellationToken::default();
        let clone = original.clone();

        assert!(!original.is_cancelled());
        clone.cancel();
        assert!(original.is_cancelled());
    }

    #[test]
    fn ensure_not_cancelled_returns_error_when_cancelled() {
        let token = CancellationToken::default();
        token.cancel();

        let err = ensure_not_cancelled(Some(&token)).expect_err("must fail");
        assert!(matches!(err, Error::Cancelled));
    }
}
