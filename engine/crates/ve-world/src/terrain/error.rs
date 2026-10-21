//! Errors of the terrain crate.

/// Why a terrain operation did not run.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TerrainError {
    /// The cancel callback returned true.
    #[error("terrain generation cancelled")]
    Cancelled,
    /// A tile side is over the limit.
    #[error("a tile side of {side} cells is over the limit of {max}; use more, smaller tiles")]
    TooLarge {
        /// Requested cells along the side.
        side: usize,
        /// The most allowed.
        max: usize,
    },
    /// A layer or window does not fit the grid.
    #[error("{what}: expected {expected} values, got {got}")]
    BadShape {
        /// What was being checked.
        what: String,
        /// Expected length or size.
        expected: usize,
        /// Length or size given.
        got: usize,
    },
    /// A parameter is outside its allowed range.
    #[error("parameter {name} is invalid: {why}")]
    BadParameter {
        /// Parameter name.
        name: String,
        /// What is wrong and how to fix it.
        why: String,
    },
    /// A layer name that does not exist, or an empty one.
    #[error("layer {name:?} is missing or has an invalid name")]
    BadLayer {
        /// The layer name.
        name: String,
    },
}
