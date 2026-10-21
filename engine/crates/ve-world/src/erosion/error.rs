//! Errors of the erosion crate.

/// Why an erosion function did not run.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ErosionError {
    /// The cancel callback returned true.
    #[error("erosion cancelled")]
    Cancelled,
    /// A grid side is over the limit.
    #[error("a side of {cells} cells is over the limit of {max}; erode in smaller tiles")]
    TooLarge {
        /// Cells along the longest side.
        cells: usize,
        /// The most allowed.
        max: usize,
    },
    /// The slice length does not match the size, or a side is under 4 cells.
    #[error(
        "grid {width} by {depth} does not match a slice of {len} heights (sides need at least 4 cells)"
    )]
    BadShape {
        /// Cells along x.
        width: usize,
        /// Cells along z.
        depth: usize,
        /// Slice length.
        len: usize,
    },
    /// A parameter is outside its allowed range.
    #[error("parameter {name} is invalid: {why}")]
    BadParameter {
        /// Parameter name.
        name: &'static str,
        /// What is wrong and how to fix it.
        why: String,
    },
}
