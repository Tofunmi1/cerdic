//! Attribute remapping for face patch dissolution.

use std::collections::BTreeMap;

use ve_kernel::ExecutionContext;

use crate::topology::check_cancelled;
use crate::{AttrData, Attribute, Domain, Element, GeometryError, PolyMesh, Topology};

pub(super) fn check_patch_attributes(
    mesh: &PolyMesh,
    selected: &std::collections::BTreeSet<usize>,
) -> Result<(), GeometryError> {
    let Some(&first) = selected.iter().next() else {
        return Err(GeometryError::PolyRefusal {
            code: "poly.selection_empty",
            element: None,
        });
    };
    for attribute in mesh.attributes().values() {
        if attribute.domain != Domain::Face {
            continue;
        }
        for &face in selected.iter().skip(1) {
            if !same_value(&attribute.data, first, face) {
                return Err(GeometryError::PolyRefusal {
                    code: "poly.attribute_conflict",
                    element: Some(Element::Face(face)),
                });
            }
        }
    }
    Ok(())
}

fn same_value(data: &AttrData, a: usize, b: usize) -> bool {
    match data {
        AttrData::F32(values) => values.get(a) == values.get(b),
        AttrData::F64(values) => values.get(a) == values.get(b),
        AttrData::I32(values) => values.get(a) == values.get(b),
        AttrData::U32(values) => values.get(a) == values.get(b),
    }
}

pub(super) fn propagate_attributes(
    source: &PolyMesh,
    candidate: &mut PolyMesh,
    source_topology: &Topology,
    candidate_topology: &Topology,
    face_sources: &[usize],
    corner_sources: &[usize],
    point_sources: &[usize],
    context: &ExecutionContext,
) -> Result<(), GeometryError> {
    let source_edges: BTreeMap<_, _> = source_topology
        .edges()
        .keys()
        .copied()
        .enumerate()
        .map(|(index, key)| (key, index))
        .collect();
    let edge_sources = if source
        .attributes()
        .values()
        .any(|attribute| attribute.domain == Domain::Edge)
    {
        remap_edges(candidate_topology, &source_edges, point_sources, context)?
    } else {
        Vec::new()
    };
    for (name, attribute) in source.attributes() {
        let indices = match attribute.domain {
            Domain::Point => Some(point_sources),
            Domain::Edge => Some(edge_sources.as_slice()),
            Domain::Face => Some(face_sources),
            Domain::Corner => Some(corner_sources),
            domain => return Err(GeometryError::DomainNotOnMesh(domain)),
        };
        let data = match indices {
            None => attribute.data.clone(),
            Some(indices) => select_attribute(&attribute.data, indices, context)?,
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

fn remap_edges(
    topology: &Topology,
    sources: &BTreeMap<(u32, u32), usize>,
    point_sources: &[usize],
    context: &ExecutionContext,
) -> Result<Vec<usize>, GeometryError> {
    let mut remapped = Vec::with_capacity(topology.edges().len());
    for (index, &(a, b)) in topology.edges().keys().enumerate() {
        if index % 512 == 0 {
            check_cancelled(context)?;
        }
        let a = *point_sources
            .get(
                usize::try_from(a)
                    .map_err(|_| GeometryError::InvalidMesh("vertex index overflow"))?,
            )
            .ok_or(GeometryError::InvalidMesh("point provenance missing"))?;
        let b = *point_sources
            .get(
                usize::try_from(b)
                    .map_err(|_| GeometryError::InvalidMesh("vertex index overflow"))?,
            )
            .ok_or(GeometryError::InvalidMesh("point provenance missing"))?;
        let edge = (
            u32::try_from(a)
                .map_err(|_| GeometryError::InvalidMesh("source vertex index overflow"))?,
            u32::try_from(b)
                .map_err(|_| GeometryError::InvalidMesh("source vertex index overflow"))?,
        );
        let edge = (edge.0.min(edge.1), edge.0.max(edge.1));
        remapped.push(*sources.get(&edge).ok_or(GeometryError::PolyRefusal {
            code: "poly.attribute_conflict",
            element: Some(Element::Edge(edge.0, edge.1)),
        })?);
    }
    Ok(remapped)
}

fn select_attribute(
    data: &AttrData,
    indices: &[usize],
    context: &ExecutionContext,
) -> Result<AttrData, GeometryError> {
    macro_rules! selected {
        ($values:expr) => {{
            let mut output = Vec::with_capacity(indices.len());
            for (index, &source) in indices.iter().enumerate() {
                if index % 512 == 0 {
                    check_cancelled(context)?;
                }
                output.push(
                    *$values
                        .get(source)
                        .ok_or(GeometryError::InvalidMesh("attribute source missing"))?,
                );
            }
            output
        }};
    }
    Ok(match data {
        AttrData::F32(values) => AttrData::F32(selected!(values)),
        AttrData::F64(values) => AttrData::F64(selected!(values)),
        AttrData::I32(values) => AttrData::I32(selected!(values)),
        AttrData::U32(values) => AttrData::U32(selected!(values)),
    })
}
