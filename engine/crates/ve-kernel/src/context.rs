//! Cooperative cancellation for the operations whose coverage is verified.
use crate::{KernelError, ffi};

/// A permanent cancellation signal. Each new evaluation needs its own context.
/// Boolean Status evaluation observes it; mesh extraction is not covered.
pub struct ExecutionContext(pub(crate) ffi::context::Context);

impl ExecutionContext {
    /// Creates an uncancelled context.
    ///
    /// # Errors
    /// Reports kernel allocation failure.
    pub fn new() -> Result<Self, KernelError> {
        ffi::context::Context::new().map(Self)
    }
    /// Requests cancellation. May be called from an observer thread in native programs.
    pub fn cancel(&self) {
        self.0.cancel();
    }
    /// Whether cancellation was requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled()
    }
}
