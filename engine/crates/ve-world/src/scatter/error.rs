//! Errors of the scatter module.

/// Why a scatter function did not run.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ScatterError {
    /// The cancel callback returned true.
    #[error("scatter cancelled")]
    Cancelled,
    /// The request would make more points than allowed.
    #[error(
        "{requested} points requested, over the limit of {max}; raise the spacing or shrink the area"
    )]
    TooMany {
        /// Points the request would make (an estimate).
        requested: usize,
        /// The most allowed.
        max: usize,
    },
    /// A channel name that does not exist, or an empty one.
    #[error("channel {name:?} is missing or has an invalid name")]
    BadChannel {
        /// The channel name.
        name: String,
    },
    /// A channel whose length is not the number of points.
    #[error("channel {name:?} has {got} values for {expected} points")]
    LengthMismatch {
        /// The channel name.
        name: String,
        /// Points in the set.
        expected: usize,
        /// Values given.
        got: usize,
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
