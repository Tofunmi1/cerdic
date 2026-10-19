//! Whole-component face orientation reversal.

use std::collections::BTreeSet;

use ve_kernel::ExecutionContext;

use crate::{AttrData, Attribute, Domain, Element, GeometryError, PolyMesh, Topology, validate};

/// Reverses selected connected components in a transactional candidate mesh.
///
/// Selection ordinals are transient input face ordinals. Persistent semantic selection is resolved
/// by the graph layer before this function is called.
///
/// # Errors
/// Returns a typed refusal when selection is empty, invalid or cuts a component; returns mesh
/// validation or cancellation errors without changing `mesh`.
pub fn flip_component(
    mesh: &PolyMesh,
    selected_faces: &[usize],
    context: &ExecutionContext,
) -> Result<PolyMesh, GeometryError> {
    let selected = checked_selection(mesh, selected_faces)?;
    validate(mesh, context)?;
    let topology = Topology::build(mesh, context)?;
    require_whole_components(&topology, &selected)?;
    let candidate = flipped_candidate(mesh, &selected, context)?;
    validate(&candidate, context)?;
    Ok(candidate)
}

/// Result of `flip_faces`, including any winding conflicts explicitly allowed by the caller.
#[derive(Clone, Debug, PartialEq)]
pub struct FlipFacesResult {
    /// Validated candidate geometry.
    pub mesh: PolyMesh,
    /// Winding conflicts remaining when `allow_inconsistent` was explicitly selected.
    pub allowed_inconsistencies: Vec<(u32, u32)>,
}

/// Reverses selected faces while preventing a partial edit from increasing winding conflicts.
///
/// Whole vertex-connected components are always accepted. `allow_inconsistent` opts into keeping
/// conflicts and returns the remaining edge keys for a warning diagnostic.
///
/// # Errors
/// Returns a typed selection or winding refusal, structural error or cancellation. The source mesh
/// remains unchanged in every case.
pub fn flip_faces(
    mesh: &PolyMesh,
    selected_faces: &[usize],
    allow_inconsistent: bool,
    context: &ExecutionContext,
) -> Result<FlipFacesResult, GeometryError> {
    let selected = checked_selection(mesh, selected_faces)?;
    let before = validate(mesh, context)?;
    if let Some(&face) = selected
        .iter()
        .find(|face| before.degenerate_faces.contains(face))
    {
        return Err(GeometryError::PolyRefusal {
            code: "poly.face_degenerate",
            element: Some(Element::Face(face)),
        });
    }
    let topology = Topology::build(mesh, context)?;
    let whole_component = is_whole_component_selection(&topology, &selected).0;
    let candidate = flipped_candidate(mesh, &selected, context)?;
    let after = validate(&candidate, context)?;
    if !whole_component
        && after.inconsistent_winding_edges.len() > before.inconsistent_winding_edges.len()
        && !allow_inconsistent
    {
        let before_edges: BTreeSet<_> = before.inconsistent_winding_edges.iter().copied().collect();
        let edge = after
            .inconsistent_winding_edges
            .iter()
            .find(|edge| !before_edges.contains(edge))
            .copied();
        return Err(GeometryError::PolyRefusal {
            code: "poly.flip_increases_inconsistency",
            element: edge.map(|(a, b)| Element::Edge(a, b)),
        });
    }
    let allowed_inconsistencies = if allow_inconsistent {
        after.inconsistent_winding_edges
    } else {
        Vec::new()
    };
    Ok(FlipFacesResult {
        mesh: candidate,
        allowed_inconsistencies,
    })
}

fn checked_selection(
    mesh: &PolyMesh,
    selected_faces: &[usize],
) -> Result<BTreeSet<usize>, GeometryError> {
    let selected: BTreeSet<usize> = selected_faces.iter().copied().collect();
    if selected.is_empty() {
        return Err(GeometryError::PolyRefusal {
            code: "poly.selection_empty",
            element: None,
        });
    }
    if let Some(&face) = selected.iter().find(|face| **face >= mesh.face_count()) {
        return Err(GeometryError::PolyRefusal {
            code: "poly.selection_invalid",
            element: Some(Element::Face(face)),
        });
    }
    Ok(selected)
}

fn flipped_candidate(
    mesh: &PolyMesh,
    selected: &BTreeSet<usize>,
    context: &ExecutionContext,
) -> Result<PolyMesh, GeometryError> {
    let reversed_corners = reverse_selected_faces(mesh, selected, context)?;
    let mut candidate = PolyMesh::new(
        mesh.positions().to_vec(),
        mesh.face_sizes().to_vec(),
        reversed_corners,
    )?;
    for (name, attribute) in mesh.attributes() {
        let data = if attribute.domain == Domain::Corner {
            reverse_corner_data(&attribute.data, mesh.face_sizes(), selected, context)?
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
    Ok(candidate)
}

fn require_whole_components(
    topology: &Topology,
    selected: &BTreeSet<usize>,
) -> Result<(), GeometryError> {
    let (whole, offending_face) = is_whole_component_selection(topology, selected);
    if !whole {
        return Err(GeometryError::PolyRefusal {
            code: "poly.selection_splits_component",
            element: offending_face.map(Element::Face),
        });
    }
    Ok(())
}

fn is_whole_component_selection(
    topology: &Topology,
    selected: &BTreeSet<usize>,
) -> (bool, Option<usize>) {
    for component in topology.components() {
        let component_faces: BTreeSet<usize> = component
            .iter()
            .flat_map(|vertex| {
                usize::try_from(*vertex)
                    .ok()
                    .and_then(|index| topology.vertex_faces().get(index))
                    .into_iter()
                    .flatten()
                    .copied()
            })
            .collect();
        let selected_count = component_faces
            .iter()
            .filter(|face| selected.contains(face))
            .count();
        if selected_count > 0 && selected_count != component_faces.len() {
            let offending_face = component_faces
                .iter()
                .find(|face| !selected.contains(face))
                .copied();
            return (false, offending_face);
        }
    }
    (true, None)
}

fn reverse_selected_faces(
    mesh: &PolyMesh,
    selected: &BTreeSet<usize>,
    context: &ExecutionContext,
) -> Result<Vec<u32>, GeometryError> {
    let mut corners = mesh.corners().to_vec();
    let mut start = 0_usize;
    for (face, &size) in mesh.face_sizes().iter().enumerate() {
        if face % 256 == 0 && context.is_cancelled() {
            return Err(GeometryError::Cancelled);
        }
        let size = usize::try_from(size)
            .map_err(|_| GeometryError::InvalidMesh("face size overflows usize"))?;
        let end = start.checked_add(size).ok_or(GeometryError::InvalidMesh(
            "face corner range overflows usize",
        ))?;
        let face_corners = corners
            .get_mut(start..end)
            .ok_or(GeometryError::InvalidMesh("face range is outside corners"))?;
        if selected.contains(&face) {
            face_corners.reverse();
        }
        start = end;
    }
    Ok(corners)
}

fn reverse_corner_data(
    data: &AttrData,
    face_sizes: &[u32],
    selected: &BTreeSet<usize>,
    context: &ExecutionContext,
) -> Result<AttrData, GeometryError> {
    let mut output = data.clone();
    let mut start = 0_usize;
    for (face, &size) in face_sizes.iter().enumerate() {
        if face % 256 == 0 && context.is_cancelled() {
            return Err(GeometryError::Cancelled);
        }
        let size = usize::try_from(size)
            .map_err(|_| GeometryError::InvalidMesh("face size overflows usize"))?;
        let end = start.checked_add(size).ok_or(GeometryError::InvalidMesh(
            "corner attribute range overflows usize",
        ))?;
        if selected.contains(&face) {
            reverse_range(&mut output, start, end)?;
        }
        start = end;
    }
    Ok(output)
}

fn reverse_range(data: &mut AttrData, start: usize, end: usize) -> Result<(), GeometryError> {
    let result = match data {
        AttrData::F32(values) => values.get_mut(start..end).map(<[f32]>::reverse),
        AttrData::F64(values) => values.get_mut(start..end).map(<[f64]>::reverse),
        AttrData::I32(values) => values.get_mut(start..end).map(<[i32]>::reverse),
        AttrData::U32(values) => values.get_mut(start..end).map(<[u32]>::reverse),
    };
    result.ok_or(GeometryError::InvalidMesh(
        "corner attribute range is outside data",
    ))
}
