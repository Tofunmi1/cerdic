//! Measurement and collision tooling for generated meshes.
//!
//! Everything here is pure Rust on plain arrays, so it runs on native and `wasm32` and any mesh
//! type can feed it: positions are `&[[f32; 3]]` (metres), indices are `&[u32]` triangles and uvs
//! are optional `&[[f32; 2]]`. A model (or a person) that builds geometry needs to check it — "is it
//! closed, how big, how thick, is it symmetric, will it print, does it have bad UVs" — and this
//! crate answers those questions with [`Report`], [`min_wall_thickness`], [`symmetry_error`],
//! [`uv_report`], [`convex_hull`], [`bounding_shapes`] and the model-facing [`measure_mesh`].
//!
//! All results are deterministic: given the same input, every function returns the same value. The
//! only randomness is the seeded, area-weighted sampling in [`min_wall_thickness`]. Budgets refuse
//! with a typed [`MeasureError`]; nothing here panics.
#![expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::many_single_char_names,
    reason = "cross product, barycentric and grid maths index with small counts and short names"
)]

mod bake;
mod bake_tool;
mod bvh;
mod collision;
mod error;
mod geom;
mod hull;
mod mesh;
mod problem;
mod report;
mod shapes;
mod symmetry;
mod thickness;
mod tool;
mod uv;

pub use bake::{Bake, LowMesh, MAX_RAYS, MAX_SIZE, MIN_SIZE, PADDING, bake_ao, bake_normals};
pub use bake_tool::bake_maps_json;
pub use bvh::{Bvh, Hausdorff, closest_point, distance_between, raycast};
pub use collision::{ConvexDecomposition, ConvexPart, convex_decomposition};
pub use error::MeasureError;
pub use geom::{ClosestPoint, RayHit};
pub use hull::{Hull, convex_hull};
pub use mesh::{BoundingBox, Mesh};
pub use problem::{Component, Problem, ProblemKind};
pub use report::Report;
pub use shapes::{BoundingShape, BoundingShapes, ShapeKind, bounding_shapes};
pub use symmetry::{Plane, SymmetryReport, best_symmetry_plane, symmetry_error};
pub use thickness::{SampleHit, Thickness, min_wall_thickness};
pub use tool::measure_mesh;
pub use uv::{UvReport, uv_report};

/// Most triangles a bounded walk accepts.
pub const MAX_TRIANGLES: usize = 1_000_000;
/// Most points the convex hull accepts.
pub const MAX_HULL_POINTS: usize = 100_000;
