//! Delete a face selection while retaining the resulting open boundary.

use std::collections::BTreeSet;

use ve_kernel::ExecutionContext;

use crate::topology::check_cancelled;
use crate::{AttrData, Attribute, Domain, Element, GeometryError, PolyMesh, Topology, validate};

/// Removes the selected faces and keeps all surviving positions and face/corner attributes.
///
/// Deletion refuses isolated points rather than silently removing their identity. Edge attributes
/// are refused because the edge set changes and no remapping policy is defined yet.
///
/// # Errors
/// Returns a typed refusal when selection is invalid, would isolate a vertex, or attributes cannot
/// be propagated; also returns structural mesh errors and cancellation.
pub fn delete_faces(
    mesh: &PolyMesh,
    selection: &[usize],
    context: &ExecutionContext,
) -> Result<PolyMesh, GeometryError> {
    let before = validate(mesh, context)?;
    let selected = selection_set(mesh.face_count(), selection)?;
    if let Some(&face) = selected
        .iter()
        .find(|face| before.degenerate_faces.contains(face))
    {
        return Err(refusal("poly.face_degenerate", face));
    }
    if mesh
        .attributes()
        .values()
        .any(|attribute| attribute.domain == Domain::Edge)
    {
        return Err(refusal("poly.attribute_conflict", selected[0]));
    }
    let topology = Topology::build(mesh, context)?;
    check_nonmanifold_touch(&topology, &selected)?;
    let (mut candidate, face_sources, corner_sources) = build_candidate(mesh, &selected, context)?;
    let used: BTreeSet<u32> = candidate.corners().iter().copied().collect();
    let originally_isolated: BTreeSet<_> = topology.isolated_vertices().iter().copied().collect();
    if let Some(vertex) = (0..mesh.positions().len())
        .filter_map(|vertex| u32::try_from(vertex).ok())
        .find(|vertex| !used.contains(vertex) && !originally_isolated.contains(vertex))
    {
        return Err(GeometryError::PolyRefusal {
            code: "poly.vertex_isolated",
            element: Some(Element::Vertex(vertex)),
        });
    }
    copy_attributes(mesh, &mut candidate, &face_sources, &corner_sources)?;
    let after = validate(&candidate, context)?;
    if after.components > before.components
        || after.nonmanifold_edges.len() > before.nonmanifold_edges.len()
        || after.inconsistent_winding_edges.len() > before.inconsistent_winding_edges.len()
        || after.degenerate_faces.len() > before.degenerate_faces.len()
    {
        return Err(refusal("poly.topology_unexpected", selected[0]));
    }
    Ok(candidate)
}

fn selection_set(count: usize, selection: &[usize]) -> Result<Vec<usize>, GeometryError> {
    let selected: BTreeSet<_> = selection.iter().copied().collect();
    if selected.is_empty() {
        return Err(GeometryError::PolyRefusal {
            code: "poly.selection_empty",
            element: None,
        });
    }
    if selected.len() != selection.len() || selected.iter().any(|&face| face >= count) {
        return Err(GeometryError::PolyRefusal {
            code: "poly.selection_invalid",
            element: selected.first().copied().map(Element::Face),
        });
    }
    if selected.len() == count {
        return Err(GeometryError::PolyRefusal {
            code: "poly.delete_all_faces",
            element: selected.first().copied().map(Element::Face),
        });
    }
    Ok(selected.into_iter().collect())
}

fn check_nonmanifold_touch(topology: &Topology, selected: &[usize]) -> Result<(), GeometryError> {
    for (&(a, b), edge) in topology.edges() {
        if edge.face_count() > 2
            && edge
                .uses()
                .iter()
                .any(|usage| selected.contains(&usage.face))
        {
            return Err(GeometryError::PolyRefusal {
                code: "poly.nonmanifold",
                element: Some(Element::Edge(a, b)),
            });
        }
    }
    Ok(())
}

fn build_candidate(
    mesh: &PolyMesh,
    selected: &[usize],
    context: &ExecutionContext,
) -> Result<(PolyMesh, Vec<usize>, Vec<usize>), GeometryError> {
    let deleted: BTreeSet<_> = selected.iter().copied().collect();
    let mut sizes = Vec::with_capacity(mesh.face_count() - selected.len());
    let mut corners = Vec::new();
    let mut faces = Vec::new();
    let mut corner_sources = Vec::new();
    let mut start = 0_usize;
    for (face, &size) in mesh.face_sizes().iter().enumerate() {
        if face % 512 == 0 {
            check_cancelled(context)?;
        }
        let end = start
            .checked_add(size as usize)
            .ok_or(GeometryError::InvalidMesh("face range overflow"))?;
        if !deleted.contains(&face) {
            sizes.push(size);
            corners.extend_from_slice(&mesh.corners()[start..end]);
            faces.push(face);
            corner_sources.extend(start..end);
        }
        start = end;
    }
    Ok((
        PolyMesh::new(mesh.positions().to_vec(), sizes, corners)?,
        faces,
        corner_sources,
    ))
}

fn copy_attributes(
    mesh: &PolyMesh,
    candidate: &mut PolyMesh,
    faces: &[usize],
    corners: &[usize],
) -> Result<(), GeometryError> {
    for (name, attribute) in mesh.attributes() {
        let data = match attribute.domain {
            Domain::Point => attribute.data.clone(),
            Domain::Face => gather(&attribute.data, faces)?,
            Domain::Corner => gather(&attribute.data, corners)?,
            Domain::Edge => return Err(GeometryError::DomainNotOnMesh(Domain::Edge)),
            Domain::CurvePoint | Domain::Instance => {
                return Err(GeometryError::DomainNotOnMesh(attribute.domain));
            }
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

fn gather(data: &AttrData, indices: &[usize]) -> Result<AttrData, GeometryError> {
    macro_rules! select {
        ($variant:ident,$values:expr) => {
            AttrData::$variant(
                indices
                    .iter()
                    .map(|&index| {
                        $values
                            .get(index)
                            .copied()
                            .ok_or(GeometryError::InvalidMesh("attribute source missing"))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            )
        };
    }
    match data {
        AttrData::F32(values) => Ok(select!(F32, values)),
        AttrData::F64(values) => Ok(select!(F64, values)),
        AttrData::I32(values) => Ok(select!(I32, values)),
        AttrData::U32(values) => Ok(select!(U32, values)),
    }
}

fn refusal(code: &'static str, face: usize) -> GeometryError {
    GeometryError::PolyRefusal {
        code,
        element: Some(Element::Face(face)),
    }
}
