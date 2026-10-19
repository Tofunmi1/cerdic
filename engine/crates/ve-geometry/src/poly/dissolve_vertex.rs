//! Dissolve an interior manifold vertex into one incident-face patch.

use std::collections::BTreeSet;

use ve_kernel::ExecutionContext;

use crate::{Element, GeometryError, PolyMesh, Topology, dissolve_faces, validate};

mod boundary;

/// Dissolves an interior manifold vertex by merging its incident face fan.
///
/// # Errors
/// Refuses isolated, boundary, or pinched vertices and names the vertex.
pub fn dissolve_vertex(
    mesh: &PolyMesh,
    vertex: u32,
    context: &ExecutionContext,
) -> Result<PolyMesh, GeometryError> {
    validate(mesh, context)?;
    let topology = Topology::build(mesh, context)?;
    let index = usize::try_from(vertex)
        .map_err(|_| GeometryError::InvalidMesh("vertex index overflows usize"))?;
    let faces = topology
        .vertex_faces()
        .get(index)
        .ok_or(GeometryError::PolyRefusal {
            code: "poly.vertex_unknown",
            element: Some(Element::Vertex(vertex)),
        })?;
    if faces.is_empty() {
        return Err(refusal("poly.vertex_isolated", vertex));
    }
    if has_multiple_face_fans(&topology, faces, context)? {
        return Err(GeometryError::PolyRefusal {
            code: "poly.nonmanifold",
            element: Some(Element::Vertex(vertex)),
        });
    }
    let mut boundary_edges = 0;
    for (&edge, info) in topology.edges() {
        if edge.0 == vertex || edge.1 == vertex {
            if info.face_count() == 1 {
                boundary_edges += 1;
            } else if info.face_count() > 2 {
                return Err(GeometryError::PolyRefusal {
                    code: "poly.nonmanifold",
                    element: Some(Element::Vertex(vertex)),
                });
            }
        }
    }
    if boundary_edges != 0 && boundary_edges != 2 {
        return Err(refusal("poly.vertex_boundary_unsupported", vertex));
    }
    match dissolve_faces(mesh, faces, context) {
        Ok(result) if boundary_edges == 2 => {
            boundary::remove_boundary_vertex(mesh, vertex, faces, &topology, result, context)
        }
        Ok(result) => Ok(result),
        Err(GeometryError::PolyRefusal {
            code: "poly.selection_disconnected",
            ..
        }) => Err(GeometryError::PolyRefusal {
            code: "poly.nonmanifold",
            element: Some(Element::Vertex(vertex)),
        }),
        Err(error) => Err(error),
    }
}

fn has_multiple_face_fans(
    topology: &Topology,
    faces: &[usize],
    context: &ExecutionContext,
) -> Result<bool, GeometryError> {
    let Some(&start) = faces.first() else {
        return Ok(false);
    };
    let incident: BTreeSet<_> = faces.iter().copied().collect();
    let mut visited = BTreeSet::from([start]);
    let mut stack = vec![start];
    while let Some(face) = stack.pop() {
        if context.is_cancelled() {
            return Err(GeometryError::Cancelled);
        }
        for &neighbor in &topology.face_neighbors()[face] {
            if incident.contains(&neighbor) && visited.insert(neighbor) {
                stack.push(neighbor);
            }
        }
    }
    Ok(visited.len() != incident.len())
}

fn refusal(code: &'static str, vertex: u32) -> GeometryError {
    GeometryError::PolyRefusal {
        code,
        element: Some(Element::Vertex(vertex)),
    }
}
