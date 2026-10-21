//! Errors of the river module.

/// Why a river, road or mask was not made.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RiverError {
    /// The cancel callback returned true.
    #[error("cancelled")]
    Cancelled,
    /// The path cannot be used.
    #[error("the path is unusable: {why}")]
    BadPath {
        /// What is wrong and how to fix it.
        why: String,
    },
    /// The slice length does not match the grid size, or a side is under 4 cells.
    #[error(
        "grid {width} by {depth} does not match a slice of {len} values (sides need at least 4 cells)"
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
