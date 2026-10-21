//! Errors of the lod module.

/// Why a plant mesh was not made.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum LodError {
    /// The cancel callback returned true.
    #[error("plant meshing cancelled")]
    Cancelled,
    /// The mesh would have more vertices than allowed.
    #[error(
        "the mesh would have {vertices} vertices, over the limit of {max}: lower radial_segments or use fewer branches"
    )]
    TooLarge {
        /// Vertices it would have (an estimate).
        vertices: usize,
        /// The most allowed.
        max: usize,
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
