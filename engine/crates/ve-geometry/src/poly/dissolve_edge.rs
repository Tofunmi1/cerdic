//! Dissolve one interior manifold edge into a simple polygon.

use std::collections::BTreeMap;

use ve_kernel::ExecutionContext;

use crate::topology::check_cancelled;
use crate::{AttrData, Attribute, Domain, Element, GeometryError, PolyMesh, Topology, validate};

/// Merges the two faces incident to an interior manifold edge.
///
/// Face data must agree across the merge. Corner values follow their source corners, and edge
/// values follow the surviving source edges.
///
/// # Errors
/// Returns a typed refusal naming the edge or face when it is unsupported. Source geometry is
/// unchanged on all failures.
pub fn dissolve_edge(
    mesh: &PolyMesh,
    edge: (u32, u32),
    context: &ExecutionContext,
) -> Result<PolyMesh, GeometryError> {
    let before = validate(mesh, context)?;
    let key = (edge.0.min(edge.1), edge.0.max(edge.1));
    let topology = Topology::build(mesh, context)?;
    let incidence = topology
        .edges()
        .get(&key)
        .ok_or(GeometryError::PolyRefusal {
            code: "poly.edge_unknown",
            element: Some(Element::Edge(key.0, key.1)),
        })?;
    if incidence.face_count() == 1 {
        return Err(GeometryError::PolyRefusal {
            code: "poly.boundary_edge",
            element: Some(Element::Edge(key.0, key.1)),
        });
    }
    if incidence.face_count() > 2 {
        return Err(GeometryError::PolyRefusal {
            code: "poly.nonmanifold",
            element: Some(Element::Edge(key.0, key.1)),
        });
    }
    let [first, second] = incidence.uses() else {
        return Err(GeometryError::PolyRefusal {
            code: "poly.boundary_edge",
            element: Some(Element::Edge(key.0, key.1)),
        });
    };
    if first.forward == second.forward {
        return Err(GeometryError::PolyRefusal {
            code: "poly.winding_conflict",
            element: Some(Element::Edge(key.0, key.1)),
        });
    }
    reject_source_slivers(&before.degenerate_faces, first.face, second.face)?;
    let (face_sizes, corners, corner_sources, face_sources, merged_vertices) =
        merge_face_loops(mesh, first.face, second.face, key, context)?;
    match crate::triangulate::validate_simple_loop(mesh, &merged_vertices, context) {
        Ok(()) => {}
        Err(GeometryError::Cancelled) => return Err(GeometryError::Cancelled),
        Err(_) => {
            return Err(GeometryError::PolyRefusal {
                code: "poly.loop_invalid",
                element: Some(Element::Edge(key.0, key.1)),
            });
        }
    }
    require_matching_face_attributes(mesh, first.face, second.face, key)?;
    let mut candidate = PolyMesh::new(mesh.positions().to_vec(), face_sizes, corners)?;
    let candidate_topology = Topology::build(&candidate, context)?;
    propagate_attributes(
        mesh,
        &mut candidate,
        &topology,
        &candidate_topology,
        &corner_sources,
        &face_sources,
        context,
    )?;
    let after = validate(&candidate, context)?;
    validate_expected_topology(&before, &after, &face_sources, key, context)?;
    Ok(candidate)
}

fn merge_face_loops(
    mesh: &PolyMesh,
    face_a: usize,
    face_b: usize,
    edge: (u32, u32),
    context: &ExecutionContext,
) -> Result<(Vec<u32>, Vec<u32>, Vec<usize>, Vec<usize>, Vec<u32>), GeometryError> {
    let loops = face_corner_ranges(mesh, context)?;
    let a = loops
        .get(face_a)
        .ok_or(GeometryError::InvalidMesh("face is missing"))?;
    let b = loops
        .get(face_b)
        .ok_or(GeometryError::InvalidMesh("face is missing"))?;
    let a_edge = find_edge_corner(mesh, a, edge, context)?;
    let b_edge = find_edge_corner(mesh, b, edge, context)?;
    let path_a = loop_path(a, a_edge, context)?;
    let path_b = loop_path(b, b_edge, context)?;
    let mut merged_sources = path_a;
    merged_sources.extend(
        path_b
            .iter()
            .skip(1)
            .take(path_b.len().saturating_sub(2))
            .copied(),
    );
    let mut distinct = std::collections::BTreeSet::new();
    let mut merged_vertices = Vec::with_capacity(merged_sources.len());
    for (index, &source_corner) in merged_sources.iter().enumerate() {
        if index % 256 == 0 {
            check_cancelled(context)?;
        }
        let vertex = mesh.corners()[source_corner];
        if !distinct.insert(vertex) {
            return Err(GeometryError::PolyRefusal {
                code: "poly.loop_invalid",
                element: Some(Element::Edge(edge.0, edge.1)),
            });
        }
        merged_vertices.push(vertex);
    }
    if merged_vertices.len() < 3 {
        return Err(GeometryError::PolyRefusal {
            code: "poly.loop_invalid",
            element: Some(Element::Edge(edge.0, edge.1)),
        });
    }
    let first_face = face_a.min(face_b);
    let second_face = face_a.max(face_b);
    let mut face_sizes = Vec::with_capacity(mesh.face_count() - 1);
    let mut corners = Vec::new();
    let mut corner_sources = Vec::new();
    let mut face_sources = Vec::with_capacity(mesh.face_count() - 1);
    for (face, range) in loops.iter().enumerate() {
        if face % 256 == 0 {
            check_cancelled(context)?;
        }
        if face == first_face {
            face_sizes.push(
                u32::try_from(merged_vertices.len())
                    .map_err(|_| GeometryError::InvalidMesh("merged face size exceeds u32"))?,
            );
            corners.extend(merged_vertices.iter().copied());
            corner_sources.extend(merged_sources.iter().copied());
            face_sources.push(first_face);
        } else if face != second_face {
            face_sizes.push(
                u32::try_from(range.len())
                    .map_err(|_| GeometryError::InvalidMesh("face size exceeds u32"))?,
            );
            corners.extend(mesh.corners()[range.clone()].iter().copied());
            corner_sources.extend(range.clone());
            face_sources.push(face);
        }
    }
    Ok((
        face_sizes,
        corners,
        corner_sources,
        face_sources,
        merged_vertices,
    ))
}

fn face_corner_ranges(
    mesh: &PolyMesh,
    context: &ExecutionContext,
) -> Result<Vec<std::ops::Range<usize>>, GeometryError> {
    let mut ranges = Vec::with_capacity(mesh.face_count());
    let mut start = 0_usize;
    for (index, &size) in mesh.face_sizes().iter().enumerate() {
        if index % 1024 == 0 {
            check_cancelled(context)?;
        }
        let size = usize::try_from(size)
            .map_err(|_| GeometryError::InvalidMesh("face size overflows usize"))?;
        let end = start
            .checked_add(size)
            .ok_or(GeometryError::InvalidMesh("face range overflows usize"))?;
        if end > mesh.corners().len() {
            return Err(GeometryError::InvalidMesh("face range exceeds corners"));
        }
        ranges.push(start..end);
        start = end;
    }
    Ok(ranges)
}

fn find_edge_corner(
    mesh: &PolyMesh,
    face: &std::ops::Range<usize>,
    edge: (u32, u32),
    context: &ExecutionContext,
) -> Result<usize, GeometryError> {
    for (index, corner) in face.clone().enumerate() {
        if index % 1024 == 0 {
            check_cancelled(context)?;
        }
        let next = if corner + 1 == face.end {
            face.start
        } else {
            corner + 1
        };
        let a = mesh.corners()[corner];
        let b = mesh.corners()[next];
        if (a.min(b), a.max(b)) == edge {
            return Ok(corner);
        }
    }
    Err(GeometryError::PolyRefusal {
        code: "poly.loop_invalid",
        element: Some(Element::Edge(edge.0, edge.1)),
    })
}

fn loop_path(
    range: &std::ops::Range<usize>,
    edge_corner: usize,
    context: &ExecutionContext,
) -> Result<Vec<usize>, GeometryError> {
    let size = range.len();
    let offset = edge_corner - range.start;
    let mut path = Vec::with_capacity(size);
    for step in 0..size {
        if step % 256 == 0 {
            check_cancelled(context)?;
        }
        path.push(range.start + (offset + 1 + step) % size);
    }
    Ok(path)
}

fn reject_source_slivers(
    slivers: &[usize],
    face_a: usize,
    face_b: usize,
) -> Result<(), GeometryError> {
    if let Some(&face) = slivers
        .iter()
        .find(|&&face| face == face_a || face == face_b)
    {
        return Err(GeometryError::PolyRefusal {
            code: "poly.face_degenerate",
            element: Some(Element::Face(face)),
        });
    }
    Ok(())
}

fn require_matching_face_attributes(
    mesh: &PolyMesh,
    face_a: usize,
    face_b: usize,
    edge: (u32, u32),
) -> Result<(), GeometryError> {
    for attribute in mesh.attributes().values() {
        if attribute.domain == Domain::Face
            && !same_attribute_value(&attribute.data, face_a, face_b)
        {
            return Err(GeometryError::PolyRefusal {
                code: "poly.attribute_conflict",
                element: Some(Element::Edge(edge.0, edge.1)),
            });
        }
    }
    Ok(())
}

fn same_attribute_value(data: &AttrData, a: usize, b: usize) -> bool {
    match data {
        AttrData::F32(values) => values.get(a) == values.get(b),
        AttrData::F64(values) => values.get(a) == values.get(b),
        AttrData::I32(values) => values.get(a) == values.get(b),
        AttrData::U32(values) => values.get(a) == values.get(b),
    }
}

fn propagate_attributes(
    source: &PolyMesh,
    candidate: &mut PolyMesh,
    source_topology: &Topology,
    candidate_topology: &Topology,
    corner_sources: &[usize],
    face_sources: &[usize],
    context: &ExecutionContext,
) -> Result<(), GeometryError> {
    let source_edges: BTreeMap<_, _> = source_topology
        .edges()
        .keys()
        .copied()
        .enumerate()
        .map(|(index, key)| (key, index))
        .collect();
    let mut edge_sources = Vec::with_capacity(candidate_topology.edges().len());
    for (index, &edge) in candidate_topology.edges().keys().enumerate() {
        if index % 1024 == 0 {
            check_cancelled(context)?;
        }
        edge_sources.push(*source_edges.get(&edge).ok_or(GeometryError::PolyRefusal {
            code: "poly.attribute_conflict",
            element: Some(Element::Edge(edge.0, edge.1)),
        })?);
    }
    for (name, attribute) in source.attributes() {
        if context.is_cancelled() {
            return Err(GeometryError::Cancelled);
        }
        let indices = match attribute.domain {
            Domain::Point => None,
            Domain::Edge => Some(edge_sources.as_slice()),
            Domain::Face => Some(face_sources),
            Domain::Corner => Some(corner_sources),
            Domain::CurvePoint | Domain::Instance => {
                return Err(GeometryError::DomainNotOnMesh(attribute.domain));
            }
        };
        let data = if let Some(indices) = indices {
            select_attribute(&attribute.data, indices, context)?
        } else {
            attribute.data.clone()
        };
        candidate.set_attribute(
            name,
            Attribute {
                domain: attribute.domain,
                data,
            },
        )?;
    }
    Ok(())
}

fn select_attribute(
    data: &AttrData,
    indices: &[usize],
    context: &ExecutionContext,
) -> Result<AttrData, GeometryError> {
    fn select<T: Copy>(
        values: &[T],
        indices: &[usize],
        context: &ExecutionContext,
    ) -> Result<Vec<T>, GeometryError> {
        let mut output = Vec::with_capacity(indices.len());
        for (index, &source) in indices.iter().enumerate() {
            if index % 1024 == 0 {
                check_cancelled(context)?;
            }
            output.push(*values.get(source).ok_or(GeometryError::InvalidMesh(
                "attribute source is outside data",
            ))?);
        }
        Ok(output)
    }
    Ok(match data {
        AttrData::F32(values) => AttrData::F32(select(values, indices, context)?),
        AttrData::F64(values) => AttrData::F64(select(values, indices, context)?),
        AttrData::I32(values) => AttrData::I32(select(values, indices, context)?),
        AttrData::U32(values) => AttrData::U32(select(values, indices, context)?),
    })
}

fn validate_expected_topology(
    before: &crate::ValidationReport,
    after: &crate::ValidationReport,
    face_sources: &[usize],
    edge: (u32, u32),
    context: &ExecutionContext,
) -> Result<(), GeometryError> {
    if before.euler_characteristic != after.euler_characteristic
        || before.components != after.components
        || after.edges + 1 != before.edges
        || after.faces + 1 != before.faces
    {
        return Err(GeometryError::PolyRefusal {
            code: "poly.topology_unexpected",
            element: Some(Element::Edge(edge.0, edge.1)),
        });
    }
    let before_slivers: std::collections::BTreeSet<_> =
        before.degenerate_faces.iter().copied().collect();
    for (new_face, &source_face) in face_sources.iter().enumerate() {
        if new_face % 1024 == 0 {
            check_cancelled(context)?;
        }
        if after.degenerate_faces.contains(&new_face) && !before_slivers.contains(&source_face) {
            return Err(GeometryError::PolyRefusal {
                code: "poly.face_degenerate",
                element: Some(Element::Face(source_face)),
            });
        }
    }
    Ok(())
}
