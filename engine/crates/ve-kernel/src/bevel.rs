//! Edge bevel on polygon meshes, using Blender's bmesh bevel.

use crate::{KernelError, ffi};

/// A borrowed indexed polygon mesh: faces of any size, vertex indices face after face.
#[derive(Clone, Copy, Debug)]
pub struct PolygonMesh<'a> {
    /// Vertex positions.
    pub positions: &'a [[f32; 3]],
    /// Corners in each face (at least three, with distinct vertices).
    pub face_sizes: &'a [u32],
    /// Vertex index per corner, face after face.
    pub corners: &'a [u32],
}

/// How the bevel width is measured.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BevelOffset {
    /// Distance each new edge moves from the original edge, along the faces.
    Offset,
    /// Width of the bevel face.
    Width,
    /// Perpendicular depth of the bevel.
    Depth,
    /// Percentage (0..=100) of the adjacent edges' length.
    Percent,
}

impl BevelOffset {
    pub(crate) const fn code(self) -> i32 {
        match self {
            Self::Offset => 0,
            Self::Width => 1,
            Self::Depth => 2,
            Self::Percent => 3,
        }
    }
}

/// Parameters of [`bevel_edges`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BevelOptions {
    /// Bevel size, greater than zero. It is clamped so the bevel never overlaps itself.
    pub width: f32,
    /// Segments across the bevel, 1 to 64.
    pub segments: u32,
    /// How `width` is measured.
    pub offset_type: BevelOffset,
    /// Profile shape, 0.1 to 1; 0.5 is a circular arc, 1 a straight chamfer-like curve.
    pub profile: f32,
}

/// The result of a bevel: a polygon mesh plus where its elements came from.
///
/// An origin of [`BeveledMesh::NO_ORIGIN`] means the element is new and inherited nothing.
#[derive(Clone, Debug, PartialEq)]
pub struct BeveledMesh {
    /// Vertex positions.
    pub positions: Vec<[f32; 3]>,
    /// Corners in each face.
    pub face_sizes: Vec<u32>,
    /// Vertex index per corner, face after face.
    pub corners: Vec<u32>,
    /// For each vertex, the input vertex whose attributes it inherits.
    pub vertex_origin: Vec<u32>,
    /// For each face, the input face whose attributes it inherits.
    pub face_parent: Vec<u32>,
    /// What the operator made each face: 0 nothing, 1 the inner face of an inset or the moved cap of an
    /// extrusion, 2 a ring face of an inset or a side wall of an extrusion.
    pub face_role: Vec<u32>,
}

impl BeveledMesh {
    /// Marks an element that descends from no input element.
    pub const NO_ORIGIN: u32 = u32::MAX;

    /// This result as a borrowed mesh, for example to bevel again.
    #[must_use]
    pub fn as_mesh(&self) -> PolygonMesh<'_> {
        PolygonMesh {
            positions: &self.positions,
            face_sizes: &self.face_sizes,
            corners: &self.corners,
        }
    }
}

/// Bevels the edges given as vertex pairs.
///
/// Every edge must exist in the mesh and join exactly two faces. Meshes are limited to 1,000,000
/// vertices, 1,000,000 faces and 4,000,000 corners, and `edges * segments` to 2,000,000.
///
/// # Errors
/// [`KernelError::InvalidArgument`] for a bad parameter, a malformed face, or a pair that is not an
/// edge; [`KernelError::OutOfBounds`] for an index past the vertices; [`KernelError::NonFinite`] for
/// a NaN or infinite coordinate; [`KernelError::NotManifold`] for an edge that does not join two
/// faces; [`KernelError::TooLarge`] past the size limits.
pub fn bevel_edges(
    mesh: &PolygonMesh<'_>,
    edges: &[[u32; 2]],
    options: &BevelOptions,
) -> Result<BeveledMesh, KernelError> {
    ffi::polymesh::bevel(mesh, edges, options)
}
