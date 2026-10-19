//! Attribute propagation for vertex welding.

use std::collections::BTreeMap;

use ve_kernel::ExecutionContext;

use crate::topology::Topology;
use crate::topology::check_cancelled;
use crate::{AttrData, Element, GeometryError};

pub(super) fn edge_source_indices(
    original: &Topology,
    candidate: &Topology,
    vertex_remap: &[u32],
    context: &ExecutionContext,
) -> Result<Vec<Vec<usize>>, GeometryError> {
    let mut source_map = BTreeMap::<(u32, u32), Vec<usize>>::new();
    for (index, &(a, b)) in original.edges().keys().enumerate() {
        if index % 1024 == 0 {
            check_cancelled(context)?;
        }
        let a_new = remap_vertex(vertex_remap, a)?;
        let b_new = remap_vertex(vertex_remap, b)?;
        if a_new != b_new {
            source_map
                .entry((a_new.min(b_new), a_new.max(b_new)))
                .or_default()
                .push(index);
        }
    }
    let mut output = Vec::with_capacity(candidate.edges().len());
    for (index, &key) in candidate.edges().keys().enumerate() {
        if index % 1024 == 0 {
            check_cancelled(context)?;
        }
        output.push(
            source_map
                .get(&key)
                .cloned()
                .ok_or(GeometryError::PolyRefusal {
                    code: "poly.attribute_conflict",
                    element: Some(Element::Edge(key.0, key.1)),
                })?,
        );
    }
    Ok(output)
}

fn remap_vertex(remap: &[u32], vertex: u32) -> Result<u32, GeometryError> {
    remap
        .get(
            usize::try_from(vertex)
                .map_err(|_| GeometryError::InvalidMesh("vertex index overflows usize"))?,
        )
        .copied()
        .ok_or(GeometryError::InvalidMesh(
            "edge vertex is missing from weld map",
        ))
}

pub(super) fn select_data(
    data: &AttrData,
    indices: &[usize],
    context: &ExecutionContext,
) -> Result<AttrData, GeometryError> {
    fn selected<T: Copy>(
        values: &[T],
        indices: &[usize],
        context: &ExecutionContext,
    ) -> Result<Vec<T>, GeometryError> {
        let mut output = Vec::with_capacity(indices.len());
        for (position, &index) in indices.iter().enumerate() {
            if position % 1024 == 0 {
                check_cancelled(context)?;
            }
            output.push(
                values
                    .get(index)
                    .copied()
                    .ok_or(GeometryError::InvalidMesh(
                        "attribute index is outside data",
                    ))?,
            );
        }
        Ok(output)
    }
    Ok(match data {
        AttrData::F32(values) => AttrData::F32(selected(values, indices, context)?),
        AttrData::F64(values) => AttrData::F64(selected(values, indices, context)?),
        AttrData::I32(values) => AttrData::I32(selected(values, indices, context)?),
        AttrData::U32(values) => AttrData::U32(selected(values, indices, context)?),
    })
}

pub(super) fn select_edge_data(
    data: &AttrData,
    sources: &[Vec<usize>],
    edge_keys: &[(u32, u32)],
    context: &ExecutionContext,
) -> Result<AttrData, GeometryError> {
    let mut selected = Vec::with_capacity(sources.len());
    for (index, candidates) in sources.iter().enumerate() {
        if index % 1024 == 0 {
            check_cancelled(context)?;
        }
        let Some(&first) = candidates.first() else {
            return Err(GeometryError::PolyRefusal {
                code: "poly.attribute_conflict",
                element: edge_keys
                    .get(index)
                    .copied()
                    .map(|(a, b)| Element::Edge(a, b)),
            });
        };
        for (candidate_index, &candidate) in candidates.iter().enumerate().skip(1) {
            if candidate_index % 1024 == 0 {
                check_cancelled(context)?;
            }
            if !same_value(data, first, candidate) {
                return Err(GeometryError::PolyRefusal {
                    code: "poly.attribute_conflict",
                    element: edge_keys
                        .get(index)
                        .copied()
                        .map(|(a, b)| Element::Edge(a, b)),
                });
            }
        }
        selected.push(first);
    }
    select_data(data, &selected, context)
}

fn same_value(data: &AttrData, first: usize, second: usize) -> bool {
    match data {
        AttrData::F32(values) => values.get(first) == values.get(second),
        AttrData::F64(values) => values.get(first) == values.get(second),
        AttrData::I32(values) => values.get(first) == values.get(second),
        AttrData::U32(values) => values.get(first) == values.get(second),
    }
}
