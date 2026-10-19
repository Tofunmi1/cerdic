//! Typed refusals, so a caller can fix the input instead of catching a panic.
//!
//! Every module in this crate returns one of these rather than panicking, and every budget is a
//! variant here, so a caller can tell "too big" apart from "malformed".

/// Everything `ve-measure` refuses to do, and why.
///
/// These are all recoverable: a caller can inspect the variant, fix the mesh or the budget, and run
/// the measurement again. No code path in this crate panics.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum MeasureError {
    /// The mesh had no triangles.
    #[error("the mesh has no triangles")]
    Empty,
    /// The index list is not a whole number of triangles.
    #[error("{0} indices is not a whole number of triangles")]
    NotTriangles(usize),
    /// A triangle index pointed outside the vertex array.
    #[error("index {index} is out of range for {vertices} vertices")]
    IndexOutOfRange {
        /// The offending index.
        index: u32,
        /// How many vertices the mesh had.
        vertices: usize,
    },
    /// A vertex coordinate was not finite.
    #[error("vertex {0} has a non-finite coordinate")]
    NonFinite(usize),
    /// A texture-coordinate list did not match the vertex count.
    #[error("{uvs} uvs for {vertices} vertices")]
    UvLength {
        /// How many uvs were supplied.
        uvs: usize,
        /// How many vertices the mesh has.
        vertices: usize,
    },
    /// The mesh is larger than a bounded tool is willing to walk.
    #[error("mesh has {triangles} triangles, above the {limit}-triangle budget")]
    TriangleBudget {
        /// How many triangles the mesh has.
        triangles: usize,
        /// The hard limit.
        limit: usize,
    },
    /// The point set is larger than the hull budget.
    #[error("point set has {points} points, above the {limit}-point budget")]
    PointBudget {
        /// How many points were supplied.
        points: usize,
        /// The hard limit.
        limit: usize,
    },
    /// A measurement needs texture coordinates and the mesh had none.
    #[error("the mesh has no uvs")]
    MissingUv,
    /// No inward ray hit anything, so the surface has no measurable wall.
    #[error("no inward ray hit a surface, so wall thickness is undefined")]
    NoThickness,
    /// The input has no interior (too few, collinear or coplanar points).
    #[error("degenerate input: {0}")]
    Degenerate(&'static str),
}
