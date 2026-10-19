//! Structural validation and reportable geometry conditions for polygon topology.

use std::collections::BTreeSet;

use ve_kernel::ExecutionContext;

use crate::topology::check_cancelled;
use crate::{GeometryError, PolyMesh, Topology};

/// Structural validation and non-fatal geometry conditions found on a mesh.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationReport {
    /// Number of points in the mesh.
    pub vertices: usize,
    /// Number of unique undirected edges.
    pub edges: usize,
    /// Number of faces.
    pub faces: usize,
    /// Vertex-connected components containing edges.
    pub components: usize,
    /// Vertices referenced by no face.
    pub isolated_vertices: usize,
    /// Closed boundary loops found.
    pub boundary_loops: usize,
    /// Face ordinals below the relative f32 area warning threshold.
    pub degenerate_faces: Vec<usize>,
    /// Sorted edge keys incident to more than two faces, with incidence count.
    pub nonmanifold_edges: Vec<(u32, u32, usize)>,
    /// Sorted edge keys whose incident faces traverse in the same direction.
    pub inconsistent_winding_edges: Vec<(u32, u32)>,
    /// Boundary edges that could not be assembled into closed loops.
    pub unclosed_boundary_edges: usize,
    /// Euler characteristic over all points, edges and faces.
    pub euler_characteristic: i64,
    /// Number of bounded face/edge/vertex records inspected.
    pub work_items: usize,
}

/// Validates structural polygon loops and reports conditions for operators to inspect.
///
/// Small nonzero faces, nonmanifold edges, inconsistent winding and incomplete boundary loops
/// are reported rather than rejected globally. Operators decide whether those conditions touch
/// their inputs and violate their own contracts.
///
/// # Errors
/// Returns a geometry error for malformed corner loops or cancellation.
pub fn validate(
    mesh: &PolyMesh,
    context: &ExecutionContext,
) -> Result<ValidationReport, GeometryError> {
    let topology = Topology::build(mesh, context)?;
    let degenerate_faces = validate_faces(mesh, context)?;
    let (nonmanifold_edges, inconsistent_winding_edges) = validate_edges(&topology, context)?;
    let boundary_edge_count = topology
        .edges()
        .values()
        .filter(|edge| edge.face_count() == 1)
        .count();
    let loop_edge_count: usize = topology.boundary_loops().iter().map(Vec::len).sum();
    let work_items = mesh
        .positions()
        .len()
        .saturating_add(mesh.corners().len())
        .saturating_add(topology.edges().len());
    Ok(ValidationReport {
        vertices: mesh.positions().len(),
        edges: topology.edges().len(),
        faces: mesh.face_count(),
        components: topology.components().len(),
        isolated_vertices: topology.isolated_vertices().len(),
        boundary_loops: topology.boundary_loops().len(),
        degenerate_faces,
        nonmanifold_edges,
        inconsistent_winding_edges,
        unclosed_boundary_edges: boundary_edge_count.saturating_sub(loop_edge_count),
        euler_characteristic: topology.euler_characteristic(),
        work_items,
    })
}

fn validate_faces(
    mesh: &PolyMesh,
    context: &ExecutionContext,
) -> Result<Vec<usize>, GeometryError> {
    let maximum_coordinate = mesh
        .positions()
        .iter()
        .flatten()
        .fold(0.0_f64, |maximum, value| {
            maximum.max(f64::from(value.abs()))
        });
    let mut corner_start = 0_usize;
    let mut degenerate_faces = Vec::new();
    for (face_id, &size) in mesh.face_sizes().iter().enumerate() {
        if face_id % 256 == 0 {
            check_cancelled(context)?;
        }
        let size = usize::try_from(size)
            .map_err(|_| GeometryError::InvalidMesh("face size overflows usize"))?;
        let face = mesh
            .corners()
            .get(corner_start..corner_start + size)
            .ok_or(GeometryError::InvalidMesh("face range is outside corners"))?;
        let distinct: BTreeSet<u32> = face.iter().copied().collect();
        if distinct.len() < 3 {
            return Err(GeometryError::InvalidMesh(
                "face has fewer than three distinct corners",
            ));
        }
        if distinct.len() != face.len() {
            return Err(GeometryError::InvalidMesh("face repeats a corner"));
        }
        if !has_resolvable_area(mesh, face, maximum_coordinate, context)? {
            degenerate_faces.push(face_id);
        }
        corner_start += size;
    }
    Ok(degenerate_faces)
}

fn has_resolvable_area(
    mesh: &PolyMesh,
    face: &[u32],
    maximum_coordinate: f64,
    context: &ExecutionContext,
) -> Result<bool, GeometryError> {
    let mut normal = [0.0_f64; 3];
    let mut minimum = [f64::INFINITY; 3];
    let mut maximum = [f64::NEG_INFINITY; 3];
    for index in 0..face.len() {
        if index % 256 == 0 {
            check_cancelled(context)?;
        }
        let a = position(mesh, face[index])?;
        let b = position(mesh, face[(index + 1) % face.len()])?;
        for axis in 0..3 {
            minimum[axis] = minimum[axis].min(a[axis]);
            maximum[axis] = maximum[axis].max(a[axis]);
        }
        normal[0] += (a[1] - b[1]) * (a[2] + b[2]);
        normal[1] += (a[2] - b[2]) * (a[0] + b[0]);
        normal[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let extent = (0..3)
        .map(|axis| maximum[axis] - minimum[axis])
        .fold(0.0_f64, f64::max);
    let area_twice = normal.iter().map(|value| value * value).sum::<f64>().sqrt();
    let resolution = f64::from(f32::EPSILON) * maximum_coordinate.max(extent);
    Ok(area_twice > 2.0 * resolution * extent)
}

fn position(mesh: &PolyMesh, index: u32) -> Result<[f64; 3], GeometryError> {
    let index = usize::try_from(index)
        .map_err(|_| GeometryError::InvalidMesh("vertex index overflows usize"))?;
    let point = mesh
        .positions()
        .get(index)
        .ok_or(GeometryError::InvalidMesh(
            "corner refers to missing vertex",
        ))?;
    Ok([
        f64::from(point[0]),
        f64::from(point[1]),
        f64::from(point[2]),
    ])
}

fn validate_edges(
    topology: &Topology,
    context: &ExecutionContext,
) -> Result<(Vec<(u32, u32, usize)>, Vec<(u32, u32)>), GeometryError> {
    let mut nonmanifold = Vec::new();
    let mut inconsistent_winding = Vec::new();
    for (index, (&key, edge)) in topology.edges().iter().enumerate() {
        if index % 1024 == 0 {
            check_cancelled(context)?;
        }
        match edge.uses() {
            [a, b] if a.forward == b.forward => inconsistent_winding.push(key),
            [_, _] | [_] => {}
            [_, _, ..] => nonmanifold.push((key.0, key.1, edge.face_count())),
            _ => return Err(GeometryError::InvalidMesh("edge has no incident faces")),
        }
    }
    Ok((nonmanifold, inconsistent_winding))
}
