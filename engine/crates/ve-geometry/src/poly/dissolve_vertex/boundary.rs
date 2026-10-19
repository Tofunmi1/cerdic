//! Boundary corner removal and domain remapping for vertex dissolution.

use std::collections::BTreeSet;

use ve_kernel::ExecutionContext;

use crate::topology::check_cancelled;
use crate::{AttrData, Attribute, Domain, Element, GeometryError, PolyMesh, Topology, validate};

pub(super) fn remove_boundary_vertex(
    source: &PolyMesh,
    vertex: u32,
    selected_faces: &[usize],
    source_topology: &Topology,
    merged: PolyMesh,
    context: &ExecutionContext,
) -> Result<PolyMesh, GeometryError> {
    if merged
        .attributes()
        .values()
        .any(|attribute| attribute.domain == Domain::Edge)
    {
        return Err(refusal("poly.attribute_conflict", vertex));
    }
    let point_sources = surviving_points(source, selected_faces, source_topology, context)?;
    let source_vertex = usize::try_from(vertex)
        .map_err(|_| GeometryError::InvalidMesh("vertex index overflows usize"))?;
    let Some(vertex_index) = point_sources
        .iter()
        .position(|&source| source == source_vertex)
    else {
        return Err(refusal("poly.loop_invalid", vertex));
    };
    let first_face = *selected_faces
        .iter()
        .min()
        .ok_or_else(|| refusal("poly.selection_empty", vertex))?;
    let mut ranges = Vec::with_capacity(merged.face_count());
    let mut start = 0_usize;
    for &size in merged.face_sizes() {
        let end = start
            .checked_add(
                usize::try_from(size)
                    .map_err(|_| GeometryError::InvalidMesh("face size overflow"))?,
            )
            .ok_or(GeometryError::InvalidMesh("face range overflow"))?;
        ranges.push(start..end);
        start = end;
    }
    let face_range = ranges
        .get(first_face)
        .ok_or_else(|| refusal("poly.loop_invalid", vertex))?;
    let corner_index = merged.corners()[face_range.clone()]
        .iter()
        .position(|&point| usize::try_from(point).ok() == Some(vertex_index))
        .map(|offset| face_range.start + offset)
        .ok_or_else(|| refusal("poly.loop_invalid", vertex))?;
    if face_range.len() <= 3 {
        return Err(refusal("poly.loop_invalid", vertex));
    }
    let result = remove_corner_and_attributes(
        merged,
        first_face,
        vertex_index,
        corner_index,
        vertex,
        context,
    )?;
    let before = validate(source, context)?;
    let after = validate(&result, context)?;
    if before.euler_characteristic != after.euler_characteristic
        || before.components != after.components
        || after.nonmanifold_edges.len() > before.nonmanifold_edges.len()
        || after.inconsistent_winding_edges.len() > before.inconsistent_winding_edges.len()
    {
        return Err(refusal("poly.topology_unexpected", vertex));
    }
    Ok(result)
}

fn remove_corner_and_attributes(
    merged: PolyMesh,
    face: usize,
    point: usize,
    corner: usize,
    vertex: u32,
    context: &ExecutionContext,
) -> Result<PolyMesh, GeometryError> {
    let mut sizes = merged.face_sizes().to_vec();
    sizes[face] -= 1;
    let mut positions = Vec::with_capacity(merged.positions().len().saturating_sub(1));
    for (index, position) in merged.positions().iter().enumerate() {
        if index % 1024 == 0 {
            check_cancelled(context)?;
        }
        if index != point {
            positions.push(*position);
        }
    }
    let mut corners = Vec::with_capacity(merged.corners().len().saturating_sub(1));
    for (index, &source) in merged.corners().iter().enumerate() {
        if index % 1024 == 0 {
            check_cancelled(context)?;
        }
        if index != corner {
            corners.push(source);
        }
    }
    for index in &mut corners {
        if usize::try_from(*index).is_ok_and(|source| source > point) {
            *index -= 1;
        }
    }
    let mut result = PolyMesh::new(positions, sizes, corners)?;
    for (name, attribute) in merged.attributes() {
        let data = match attribute.domain {
            Domain::Point => drop_value(&attribute.data, point, context)?,
            Domain::Corner => drop_value(&attribute.data, corner, context)?,
            Domain::Face => attribute.data.clone(),
            Domain::Edge => return Err(refusal("poly.attribute_conflict", vertex)),
            domain => return Err(GeometryError::DomainNotOnMesh(domain)),
        };
        result.set_attribute(
            name,
            Attribute {
                domain: attribute.domain,
                data,
            },
        )?;
    }
    Ok(result)
}

fn surviving_points(
    mesh: &PolyMesh,
    selected_faces: &[usize],
    topology: &Topology,
    context: &ExecutionContext,
) -> Result<Vec<usize>, GeometryError> {
    let selected: BTreeSet<_> = selected_faces.iter().copied().collect();
    let mut used = BTreeSet::new();
    let mut ranges = Vec::with_capacity(mesh.face_count());
    let mut start = 0_usize;
    for &size in mesh.face_sizes() {
        let end = start
            .checked_add(
                usize::try_from(size)
                    .map_err(|_| GeometryError::InvalidMesh("face size overflow"))?,
            )
            .ok_or(GeometryError::InvalidMesh("face range overflow"))?;
        ranges.push(start..end);
        start = end;
    }
    for (face, range) in ranges.iter().enumerate() {
        if face % 256 == 0 {
            check_cancelled(context)?;
        }
        if !selected.contains(&face) {
            for &point in &mesh.corners()[range.clone()] {
                used.insert(
                    usize::try_from(point)
                        .map_err(|_| GeometryError::InvalidMesh("vertex index overflow"))?,
                );
            }
        }
    }
    for (index, (&(a, b), info)) in topology.edges().iter().enumerate() {
        if index % 512 == 0 {
            check_cancelled(context)?;
        }
        let count = info
            .uses()
            .iter()
            .filter(|usage| selected.contains(&usage.face))
            .count();
        if count == 1 {
            used.insert(
                usize::try_from(a)
                    .map_err(|_| GeometryError::InvalidMesh("vertex index overflow"))?,
            );
            used.insert(
                usize::try_from(b)
                    .map_err(|_| GeometryError::InvalidMesh("vertex index overflow"))?,
            );
        }
    }
    Ok(used.into_iter().collect())
}

fn drop_value(
    data: &AttrData,
    removed: usize,
    context: &ExecutionContext,
) -> Result<AttrData, GeometryError> {
    macro_rules! drop_one {
        ($values:expr) => {{
            if removed >= $values.len() {
                return Err(GeometryError::InvalidMesh(
                    "attribute removal index is missing",
                ));
            }
            let mut values = Vec::with_capacity($values.len().saturating_sub(1));
            for (index, &value) in $values.iter().enumerate() {
                if index % 1024 == 0 {
                    check_cancelled(context)?;
                }
                if index != removed {
                    values.push(value);
                }
            }
            values
        }};
    }
    Ok(match data {
        AttrData::F32(values) => AttrData::F32(drop_one!(values)),
        AttrData::F64(values) => AttrData::F64(drop_one!(values)),
        AttrData::I32(values) => AttrData::I32(drop_one!(values)),
        AttrData::U32(values) => AttrData::U32(drop_one!(values)),
    })
}

fn refusal(code: &'static str, vertex: u32) -> GeometryError {
    GeometryError::PolyRefusal {
        code,
        element: Some(Element::Vertex(vertex)),
    }
}
