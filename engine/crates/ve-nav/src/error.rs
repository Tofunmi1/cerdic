//! Why building or querying a navigation mesh failed.

/// Everything that can go wrong when building or querying a [`crate::NavMesh`].
///
/// All of these are recoverable: a level generator can catch one, fix the input (or the config) and
/// try again, and no code path panics.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum NavError {
    /// A configuration value was not a usable number.
    #[error("{field} must be finite, got {value}")]
    NotFinite {
        /// Which setting was wrong.
        field: &'static str,
        /// The value that was supplied.
        value: f32,
    },
    /// The cell size was not positive.
    #[error("cell size must be greater than zero, got {0}")]
    BadCellSize(f32),
    /// The agent radius was negative.
    #[error("agent radius must not be negative, got {0}")]
    BadAgentRadius(f32),
    /// The agent height was not positive.
    #[error("agent height must be greater than zero, got {0}")]
    BadAgentHeight(f32),
    /// The maximum step was negative.
    #[error("max step must not be negative, got {0}")]
    BadMaxStep(f32),
    /// An agent's top speed was negative.
    #[error("agent max speed must not be negative, got {0}")]
    BadSpeed(f32),
    /// The maximum slope was outside `(0, 90)` degrees.
    #[error("max slope must be in (0, 90) degrees, got {0}")]
    BadMaxSlope(f32),
    /// The bounds were empty or inverted in `x` or `z`.
    #[error("bounds {min:?}..{max:?} must be a non-empty box in x and z")]
    BadBounds {
        /// Lower corner.
        min: [f32; 3],
        /// Upper corner.
        max: [f32; 3],
    },
    /// The mesh had no triangles.
    #[error("the mesh has no triangles")]
    EmptyMesh,
    /// The index list was not a whole number of triangles.
    #[error("{indices} indices is not a whole number of triangles")]
    BadIndexCount {
        /// The rejected index count.
        indices: usize,
    },
    /// A triangle index pointed outside the vertex array.
    #[error("triangle index {index} is out of range for {vertices} vertices")]
    BadIndex {
        /// The offending index.
        index: u32,
        /// How many vertices the mesh had.
        vertices: usize,
    },
    /// The level needs more cells than the budget allows.
    #[error("level needs {cells} cells, the limit is {max}")]
    TooManyCells {
        /// How many cells the configuration would need.
        cells: usize,
        /// The hard limit.
        max: usize,
    },
    /// No walkable cell was near the given point.
    #[error("no walkable cell near {at:?}")]
    NoWalkableCell {
        /// The query point.
        at: [f32; 3],
    },
    /// A* found no route between the snapped start and end cells.
    #[error("no path between {start:?} and {end:?} (blocked or disconnected)")]
    NoPath {
        /// The requested start.
        start: [f32; 3],
        /// The requested end.
        end: [f32; 3],
    },
    /// The crowd already holds [`crate::MAX_AGENTS`] agents.
    #[error("crowd is full ({max} agents)")]
    CrowdFull {
        /// The agent limit.
        max: usize,
    },
    /// An agent was addressed by an index that does not exist.
    #[error("no agent with id {id}")]
    NoAgent {
        /// The rejected id.
        id: usize,
    },
}
