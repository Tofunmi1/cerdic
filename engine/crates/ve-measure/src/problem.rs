//! What can be wrong with a mesh, how many times, and how to fix it.

use crate::mesh::BoundingBox;

/// The kind of topological problem a [`crate::Report`] found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProblemKind {
    /// An edge used by exactly one triangle (a hole boundary).
    OpenEdge,
    /// An edge used by three or more triangles.
    NonManifoldEdge,
    /// Two vertices share a position but not an index.
    DuplicateVertex,
    /// A triangle whose area is below the epsilon.
    DegenerateTriangle,
    /// An edge whose length is below the epsilon.
    ZeroLengthEdge,
}

impl ProblemKind {
    /// A short plural noun for the problem.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::OpenEdge => "open edges",
            Self::NonManifoldEdge => "non-manifold edges",
            Self::DuplicateVertex => "duplicate vertices",
            Self::DegenerateTriangle => "degenerate triangles",
            Self::ZeroLengthEdge => "zero-length edges",
        }
    }

    /// A suggested fix, phrased for a language model.
    #[must_use]
    pub fn advice(self) -> &'static str {
        match self {
            Self::OpenEdge => "close the hole or use poly.fill",
            Self::NonManifoldEdge => "split the fan so each edge has two faces (poly.dissolve)",
            Self::DuplicateVertex => "weld the duplicate vertices (poly.weld)",
            Self::DegenerateTriangle => "delete the slivers (poly.delete)",
            Self::ZeroLengthEdge => "weld the coincident vertices (poly.weld)",
        }
    }
}

/// One kind of problem, how many there are, and a representative location.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Problem {
    /// What is wrong.
    pub kind: ProblemKind,
    /// How many times it occurs.
    pub count: usize,
    /// A representative location, in metres.
    pub example: Option<[f32; 3]>,
}

/// One connected component of the mesh.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Component {
    /// The component's own bounding box.
    pub bounding_box: BoundingBox,
    /// The component's signed volume, in cubic metres.
    pub volume: f32,
    /// How many triangles the component has.
    pub triangles: usize,
}
