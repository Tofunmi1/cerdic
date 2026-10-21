//! Errors of the flora module.

/// Why a plant was not grown.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum FloraError {
    /// The cancel callback returned true.
    #[error("plant growth cancelled")]
    Cancelled,
    /// The plant would have more nodes than allowed.
    #[error("the plant would have {nodes} nodes, over the limit of {max}: {hint}")]
    TooLarge {
        /// Nodes it would have (an estimate).
        nodes: usize,
        /// The most allowed.
        max: usize,
        /// Which parameter to lower.
        hint: String,
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
