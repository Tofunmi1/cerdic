//! Mesh operators on polygon meshes: inset, solidify, extrude, dissolve, poke, subdivide, wireframe, symmetrize,
//! bisect and the rest, implemented in Rust (see `native/`).

use crate::{BeveledMesh, KernelError, PolygonMesh};

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

/// One of the mesh operators, with its parameters.
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
    /// Splits every selected polygon into triangles.
    Triangulate,
    /// Merges selected vertices within a bounded distance.
    RemoveDoubles {
        /// Maximum distance between vertices to merge.
        distance: f64,
    },
    /// Bisects the entire mesh with a plane.
    Bisect {
        /// A point on the plane.
        point: [f64; 3],
        /// The plane normal.
        normal: [f64; 3],
        /// Distance for classifying vertices on the plane.
        distance: f64,
        /// Remove the side opposite the normal.
        clear_inner: bool,
        /// Remove the side the normal points toward.
        clear_outer: bool,
        /// Fill the cut boundary when a side is removed.
        fill: bool,
    },
    /// Revolves selected mesh geometry around an axis.
    Spin {
        /// Point on the rotation axis.
        center: [f64; 3],
        /// Rotation axis direction.
        axis: [f64; 3],
        /// Total rotation in radians.
        angle: f64,
        /// Number of rotation segments.
        steps: u32,
        /// Weld the seam for a full turn.
        merge: bool,
        /// Maximum distance between vertices to merge.
        distance: f64,
    },
    /// Adds a reflected copy of the entire mesh across a plane.
    Mirror {
        /// Point on the reflection plane.
        point: [f64; 3],
        /// Plane normal.
        normal: [f64; 3],
        /// Weld vertices within `distance` of the plane.
        merge: bool,
        /// Maximum merge distance.
        distance: f64,
    },
    /// Cuts a closed edge ring through quad faces.
    LoopCut {
        /// Point in metres whose nearest edge chooses the ring.
        at: [f64; 3],
        /// Number of cuts, from 1 to 32.
        cuts: u32,
        /// Subdivision smoothness, from 0 to 1.
        smooth: f64,
    },
    /// Points every face outward (or inward) consistently, whatever the winding was.
    RecalcNormals {
        /// Point the faces inward instead of outward.
        inside: bool,
    },
    /// Moves each vertex toward the average of its neighbours, evening out the surface.
    Smooth {
        /// How far each pass moves a vertex toward its neighbours, 0 to 1.
        factor: f64,
        /// Passes, 1 to 50.
        iterations: u32,
    },
    /// Fills boundary loops with faces.
    FillHoles {
        /// The most sides a hole may have and still be filled, 3 to 1000.
        sides: u32,
    },
    /// Rotates the edges between triangle pairs so the triangles are more even.
    Beautify {
        /// Judge by angle instead of area.
        by_angle: bool,
    },
    /// Flattens each face toward its best-fit plane.
    PlanarFaces {
        /// Passes, 1 to 50 (neighbouring faces pull on each other).
        iterations: u32,
        /// How far each pass moves vertices toward the plane, 0 to 1.
        factor: f64,
    },
    /// Joins triangle pairs into quads when the result is flat and square enough.
    JoinTriangles {
        /// Largest angle in radians between the two triangles' normals, 0 to pi.
        face_angle: f64,
        /// Largest deviation in radians of the quad's corners from right angles, 0 to pi.
        shape_angle: f64,
    },
    /// Removes edges and faces shorter or smaller than a distance.
    DissolveDegenerate {
        /// Distance in metres under which an edge or face counts as degenerate.
        distance: f64,
    },
    /// Disconnects the faces along the selected edges, so each side gets its own vertices and the edge becomes a hard seam.
    SplitEdges,
    /// Spins each selected edge to the other diagonal of the two faces it joins.
    RotateEdges {
        /// Turn counter-clockwise instead of clockwise.
        counter_clockwise: bool,
    },
    /// Adds edges between the selected vertices of each face, splitting the face.
    ConnectVertices {
        /// Refuse splits that would overlap or intersect.
        check_degenerate: bool,
    },
    /// Moves the vertices of selected edge loops to even spacing along each loop's own curve.
    SpaceEdgeLoops {
        /// How far toward even spacing, 0 to 1.
        factor: f64,
        /// Interpolate linearly instead of with a cubic curve.
        linear: bool,
    },
    /// Smooths selected edge loops.
    RelaxEdgeLoops {
        /// Passes, 1 to 50.
        iterations: u32,
        /// Interpolate linearly instead of with a cubic curve.
        linear: bool,
        /// Also space the vertices evenly.
        even_spacing: bool,
    },
    /// Reshapes selected edge loops into circles.
    Circularize {
        /// How far toward a circle, 0 to 1.
        factor: f64,
        /// Radius in metres, or 0 to fit one to the loop.
        radius: f64,
        /// Rotation of the vertices around the circle, in radians.
        angle: f64,
        /// How far the loop is flattened onto its best-fit plane, 0 to 1.
        flatten: f64,
        /// Space the vertices evenly around the circle.
        regular: bool,
    },
}

/// Runs a mesh operator.
///
/// Inset, extrude, poke, wireframe and solidify take faces (poke, wireframe and solidify treat an empty
/// selection as every face); subdivide takes edges; dissolve and symmetrize take the whole mesh.
///
/// # Errors
/// [`KernelError::InvalidArgument`] for a bad parameter, a malformed face, a missing selection or an
/// edge that is not in the mesh; [`KernelError::OutOfBounds`] for an index past the faces or vertices;
/// [`KernelError::NonFinite`] for a NaN or huge number; [`KernelError::TooLarge`] past the size limits;
/// [`KernelError::OperatorRefused`] when the operator declines the mesh (for example solidify on a mesh it
/// cannot offset, or a symmetry axis for decimate).
pub fn edit_mesh(
    mesh: &PolygonMesh<'_>,
    selection: &MeshSelection<'_>,
    op: &MeshOp,
) -> Result<EditedMesh, KernelError> {
    crate::native::shape::edit(mesh, selection, op)
        .or_else(|| crate::native::basic::edit(mesh, selection, op))
        .or_else(|| crate::native::more::edit(mesh, selection, op))
        .unwrap_or(Err(KernelError::OperatorRefused))
}
