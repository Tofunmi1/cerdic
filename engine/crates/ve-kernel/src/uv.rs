//! UV unwrapping (least-squares conformal maps) and island packing, in Rust (see `native/uv.rs`).

use crate::{KernelError, PolygonMesh};

/// How an island is flattened.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UvMethod {
    /// Least-squares conformal maps: fast, angle preserving.
    Conformal,
    /// Angle-based flattening: the same solver as [`UvMethod::Conformal`] today.
    AngleBased,
}

/// Parameters of [`unwrap_uvs`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnwrapOptions {
    /// Flattening method.
    pub method: UvMethod,
    /// Pack the islands into the unit square. Without it they overlap near the origin.
    pub pack: bool,
    /// Let packing rotate islands to fit tighter.
    pub rotate: bool,
    /// Space between packed islands, as a fraction of the unit square (0 to 0.5).
    pub margin: f32,
}

/// The result of an unwrap.
#[derive(Clone, Debug, PartialEq)]
pub struct UvLayout {
    /// One `(u, v)` per mesh corner, in the mesh's corner order.
    pub uvs: Vec<[f32; 2]>,
    /// Islands that could not be flattened (a closed piece with no seam, a degenerate one); their corners keep `(0, 0)`.
    pub failed_islands: u32,
}

/// Packing options for [`pack_uvs`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UvPackOptions {
    /// Allow islands to rotate.
    pub rotate: bool,
    /// Keep the original island scale where possible.
    pub keep_density: bool,
    /// Use axis aligned bounds or convex hulls during packing (convex hulls pack as bounds today).
    pub shape: UvPackShape,
    /// Fractional spacing between islands.
    pub margin: f32,
}

/// Island packing shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UvPackShape {
    /// Use axis-aligned bounds.
    Aabb,
    /// Use the convex hull of each island.
    Convex,
}

/// Packs an existing corner UV map into the unit square. Discontinuities define island borders.
///
/// # Errors
/// [`KernelError::InvalidArgument`] for a bad margin, a malformed face or a missing UV; [`KernelError::NonFinite`] for a
/// NaN or infinite value; [`KernelError::OutOfBounds`] for an index past the vertices; [`KernelError::TooLarge`] past
/// 1,000,000 vertices, faces or 4,000,000 corners.
pub fn pack_uvs(
    mesh: &PolygonMesh<'_>,
    input_uvs: &[[f32; 2]],
    options: &UvPackOptions,
) -> Result<Vec<[f32; 2]>, KernelError> {
    crate::native::uv::pack_uvs(mesh, input_uvs, options).unwrap_or(Err(KernelError::OperatorRefused))
}

/// Cuts the mesh along `seams` (vertex pairs, each an edge of the mesh) and flattens every island.
///
/// # Errors
/// [`KernelError::InvalidArgument`] for a bad option, a malformed face, or a seam that is not an edge;
/// [`KernelError::OutOfBounds`] for an index past the vertices; [`KernelError::NonFinite`] for a NaN or
/// infinite coordinate; [`KernelError::TooLarge`] past 1,000,000 vertices, faces or 4,000,000 corners;
/// [`KernelError::OperatorRefused`] when an edge joins more than two faces.
pub fn unwrap_uvs(
    mesh: &PolygonMesh<'_>,
    seams: &[[u32; 2]],
    options: &UnwrapOptions,
) -> Result<UvLayout, KernelError> {
    crate::native::uv::unwrap_uvs(mesh, seams, options).unwrap_or(Err(KernelError::OperatorRefused))
}
