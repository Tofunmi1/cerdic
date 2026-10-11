//! Platform errors.

/// Why the web platform could not be set up.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PlatformError {
    /// There is no `window` or `document` (not running in a browser main thread).
    #[error("no browser window or document is available")]
    NoWindow,
    /// No element has the requested id.
    #[error("no element with id {0:?}")]
    CanvasNotFound(String),
    /// The element exists but is not a `<canvas>`.
    #[error("element {0:?} is not a canvas")]
    NotACanvas(String),
}
