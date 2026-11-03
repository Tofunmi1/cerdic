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
    /// Too many named parts were supplied to the attachment check.
    #[error("attachment check has {parts} parts, above the {limit}-part budget")]
    PartBudget {
        /// Number of supplied parts.
        parts: usize,
        /// Maximum accepted part count.
        limit: usize,
    },
    /// A bounded measurement ran out of its work allowance before it finished.
    #[error("the measurement needs more than its {limit}-step work budget; check fewer or smaller parts")]
    WorkBudget {
        /// The work allowance that ran out (closest-point queries and ray casts).
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
    /// A bake setting or its low-poly UVs are not acceptable.
    #[error("bake: {0}")]
    Bake(&'static str),
    /// A sampled motion measurement received too few frames.
    #[error("{measurement} needs at least 2 frames, got {frames}")]
    MotionFrames {
        /// The measurement that refused the input.
        measurement: &'static str,
        /// Number of frames supplied.
        frames: usize,
    },
    /// A sampled motion measurement received invalid timing or contact settings.
    #[error("{measurement}: {setting} must be {requirement}")]
    MotionSetting {
        /// The measurement that refused the input.
        measurement: &'static str,
        /// Setting name.
        setting: &'static str,
        /// Required property.
        requirement: &'static str,
    },
    /// A sampled motion measurement received a non-finite coordinate.
    #[error("{measurement}: frame {frame}, point {point} has a non-finite coordinate")]
    MotionNonFinite {
        /// The measurement that refused the input.
        measurement: &'static str,
        /// Frame index.
        frame: usize,
        /// Point index.
        point: usize,
    },
    /// Loop samples do not track the same points in each frame.
    #[error("loop_seam: frame {frame} has {actual} points, expected {expected}")]
    MotionPointCount {
        /// Offending frame index.
        frame: usize,
        /// Actual point count.
        actual: usize,
        /// Expected point count.
        expected: usize,
    },
    /// Loop samples contain no tracked points.
    #[error("loop_seam needs at least 1 tracked point")]
    MotionNoPoints,
    /// Bone endpoints do not have matching frame counts.
    #[error("length_drift needs matching endpoint frame counts, got {a} and {b}")]
    MotionLengthMismatch {
        /// Number of first-end samples.
        a: usize,
        /// Number of second-end samples.
        b: usize,
    },
    /// A bone has zero length in the reference frame.
    #[error("length_drift needs a non-zero first-frame bone length")]
    MotionZeroLength,
}
