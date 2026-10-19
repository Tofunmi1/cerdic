//! Blender's bmesh operators on polygon meshes: inset, solidify, extrude, dissolve, poke, subdivide,
//! wireframe and symmetrize.

use crate::{BeveledMesh, KernelError, PolygonMesh, ffi};

/// The result of a mesh operator: the same layout and provenance as [`bevel_edges`](crate::bevel_edges).
pub type EditedMesh = BeveledMesh;

/// The elements an operator works on. Faces are indices into the mesh's faces; edges are vertex pairs.
#[derive(Clone, Copy, Debug, Default)]
pub struct MeshSelection<'a> {
    /// Selected faces. Empty means every face where the operator allows it.
    pub faces: &'a [u32],
    /// Selected edges, each an edge of the mesh.
    pub edges: &'a [[u32; 2]],
    /// Selected vertices.
    pub vertices: &'a [u32],
}

/// Where a poke puts its new vertex.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PokeCenter {
    /// The mean of the face's corners.
    Median,
    /// The mean weighted by corner angle, which suits uneven polygons.
    Weighted,
    /// The centre of the face's bounding box.
    Bounds,
}

/// One of Blender's mesh operators, with its parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeshOp {
    /// Shrinks each selected face region inward, leaving a ring of faces around it.
    Inset {
        /// Width of the ring, greater than zero.
        thickness: f64,
        /// How far the inner face is raised along its normal (negative sinks it).
        depth: f64,
        /// Scale the offset so the ring has even thickness at corners.
        even: bool,
        /// Scale the offset by the surrounding geometry.
        relative: bool,
        /// Also inset the region's outer boundary.
        boundary: bool,
        /// Grow outward instead of shrinking inward.
        outset: bool,
    },
    /// Gives the surface thickness: a second surface offset along the normals, joined at open edges.
    Solidify {
        /// Distance along each face's normal, non-zero (negative goes inward).
        thickness: f64,
    },
    /// Extrudes the selected faces and moves the new faces by an offset.
    Extrude {
        /// The offset the new faces move by.
        offset: [f64; 3],
    },
    /// Merges neighbouring faces that are flatter than an angle.
    Dissolve {
        /// Faces whose normals differ by less than this, in radians (0 to pi), merge.
        angle: f64,
        /// Also dissolve along boundaries.
        boundaries: bool,
    },
    /// Adds a vertex in each selected face and fans triangles to it.
    Poke {
        /// How far the new vertex is raised along the normal.
        offset: f64,
        /// Where it sits in the face.
        center: PokeCenter,
        /// Scale the offset by the face's size.
        relative: bool,
    },
    /// Cuts the selected edges into pieces and splits the faces around them.
    Subdivide {
        /// Cuts per edge, 1 to 8.
        cuts: u32,
        /// How much the new points are pushed to smooth the surface, 0 to 1.
        smooth: f64,
        /// Fill quads with a grid.
        grid_fill: bool,
    },
    /// Turns the faces into a lattice of struts.
    Wireframe {
        /// Strut thickness, greater than zero.
        thickness: f64,
        /// Offset of the struts along the normal.
        offset: f64,
        /// Scale thickness so struts keep an even width at corners.
        even: bool,
        /// Scale thickness by the surrounding geometry.
        relative: bool,
        /// Also make struts along boundaries.
        boundary: bool,
        /// Remove the original faces.
        replace: bool,
    },
    /// Collapses the edges that change the shape least until a fraction of the faces remains: level of
    /// detail for distant or cheap versions of a model.
    Decimate {
        /// Fraction of the faces to keep, greater than 0 and at most 1.
        ratio: f64,
        /// An axis (0 x, 1 y, 2 z) to keep the result mirror-symmetric about, or `None`.
        symmetry: Option<u8>,
        /// Triangulate first; needed unless the mesh is already triangles.
        triangulate: bool,
    },
    /// Reverses a subdivision: removes the vertices a regular grid of quads could be rebuilt without.
    Unsubdivide {
        /// How many levels to undo, 1 to 4.
        iterations: u32,
    },
    /// Keeps one side of an axis plane and mirrors it onto the other.
    Symmetrize {
        /// The axis (0 x, 1 y, 2 z) of the mirror plane.
        axis: u8,
        /// Keep the positive side (true) or the negative side (false).
        keep_positive: bool,
        /// Vertices closer than this to the plane (or to their mirror) are merged.
        distance: f64,
    },
    /// Splits every selected polygon into triangles using Blender's beauty triangulation.
    Triangulate,
    /// Merges selected vertices within a bounded distance.
    RemoveDoubles {
        /// Maximum distance between vertices to merge.
        distance: f64,
    },
}

impl MeshOp {
    /// The operator code, parameters and flags in the layout `ve_kernel.h` documents.
    #[expect(
        clippy::too_many_lines,
        reason = "one flat match over every operator keeps the ABI layout in a single place"
    )]
    fn encode(&self) -> (i32, [f64; 4], i32) {
        let bits =
            |pairs: &[(bool, i32)]| pairs.iter().filter(|(on, _)| *on).map(|(_, bit)| bit).sum();
        match *self {
            Self::Inset {
                thickness,
                depth,
                even,
                relative,
                boundary,
                outset,
            } => (
                0,
                [thickness, depth, 0.0, 0.0],
                bits(&[(even, 1), (relative, 2), (boundary, 4), (outset, 8)]),
            ),
            Self::Solidify { thickness } => (1, [thickness, 0.0, 0.0, 0.0], 0),
            Self::Extrude { offset } => (2, [offset[0], offset[1], offset[2], 0.0], 0),
            Self::Dissolve { angle, boundaries } => {
                (3, [angle, 0.0, 0.0, 0.0], bits(&[(boundaries, 1)]))
            }
            Self::Poke {
                offset,
                center,
                relative,
            } => (
                4,
                [
                    offset,
                    match center {
                        PokeCenter::Median => 0.0,
                        PokeCenter::Weighted => 1.0,
                        PokeCenter::Bounds => 2.0,
                    },
                    0.0,
                    0.0,
                ],
                bits(&[(relative, 1)]),
            ),
            Self::Subdivide {
                cuts,
                smooth,
                grid_fill,
            } => (
                5,
                [f64::from(cuts), smooth, 0.0, 0.0],
                bits(&[(grid_fill, 1)]),
            ),
            Self::Wireframe {
                thickness,
                offset,
                even,
                relative,
                boundary,
                replace,
            } => (
                6,
                [thickness, offset, 0.0, 0.0],
                bits(&[(even, 1), (relative, 2), (boundary, 4), (replace, 8)]),
            ),
            Self::Decimate {
                ratio,
                symmetry,
                triangulate,
            } => (
                8,
                [
                    ratio,
                    symmetry.map_or(-1.0, |axis| f64::from(axis.min(2))),
                    0.0,
                    0.0,
                ],
                bits(&[(triangulate, 1)]),
            ),
            Self::Unsubdivide { iterations } => (9, [f64::from(iterations), 0.0, 0.0, 0.0], 0),
            Self::Symmetrize {
                axis,
                keep_positive,
                distance,
            } => (
                7,
                [
                    f64::from(axis.min(2)) + if keep_positive { 3.0 } else { 0.0 },
                    distance,
                    0.0,
                    0.0,
                ],
                0,
            ),
            Self::Triangulate => (10, [0.0; 4], 0),
            Self::RemoveDoubles { distance } => (11, [distance, 0.0, 0.0, 0.0], 0),
        }
    }
}

/// Runs a Blender mesh operator.
///
/// Inset, extrude, poke, wireframe and solidify take faces (poke, wireframe and solidify treat an empty
/// selection as every face); subdivide takes edges; dissolve and symmetrize take the whole mesh. Meshes
/// are limited to 250,000 faces for these operators.
///
/// # Errors
/// [`KernelError::InvalidArgument`] for a bad parameter, a malformed face, a missing selection or an
/// edge that is not in the mesh; [`KernelError::OutOfBounds`] for an index past the faces or vertices;
/// [`KernelError::NonFinite`] for a NaN or huge number; [`KernelError::TooLarge`] past the size limits;
/// [`KernelError::OperatorRefused`] when Blender declines the mesh (for example solidify on a mesh it
/// cannot offset).
pub fn edit_mesh(
    mesh: &PolygonMesh<'_>,
    selection: &MeshSelection<'_>,
    op: &MeshOp,
) -> Result<EditedMesh, KernelError> {
    let (code, params, flags) = op.encode();
    ffi::polymesh::mesh_op(mesh, selection, code, params, flags)
}
