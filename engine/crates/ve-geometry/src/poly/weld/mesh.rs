//! Face rewrite and candidate validation for weld.

use std::collections::BTreeSet;

use ve_kernel::ExecutionContext;

use crate::topology::check_cancelled;
use crate::{Attribute, Domain, Element, GeometryError, PolyMesh, Topology, validate};

use super::attributes::{edge_source_indices, select_data, select_edge_data};

pub(super) fn rewrite_faces(
    mesh: &PolyMesh,
    remap: &[u32],
    context: &ExecutionContext,
) -> Result<(Vec<u32>, Vec<u32>, Vec<usize>), GeometryError> {
    let mut face_sizes = Vec::with_capacity(mesh.face_count());
    let mut corners = Vec::new();
    let mut corner_sources = Vec::new();
    let mut start = 0_usize;
    for (face, &raw_size) in mesh.face_sizes().iter().enumerate() {
        if face % 256 == 0 {
            check_cancelled(context)?;
        }
        let size = usize::try_from(raw_size)
            .map_err(|_| GeometryError::InvalidMesh("face size overflows usize"))?;
        let end = start.checked_add(size).ok_or(GeometryError::InvalidMesh(
            "face corner range overflows usize",
        ))?;
        let source = mesh
            .corners()
            .get(start..end)
            .ok_or(GeometryError::InvalidMesh("face range is outside corners"))?;
        let mut ring = Vec::with_capacity(size);
        for (offset, &vertex) in source.iter().enumerate() {
            if offset % 1024 == 0 {
                check_cancelled(context)?;
            }
            let mapped = *remap
                .get(
                    usize::try_from(vertex)
                        .map_err(|_| GeometryError::InvalidMesh("vertex index overflows usize"))?,
                )
                .ok_or(GeometryError::InvalidMesh(
                    "corner refers to missing vertex",
                ))?;
            if ring.last().is_none_or(|(previous, _)| *previous != mapped) {
                ring.push((mapped, start + offset));
            }
        }
        if ring.len() > 1
            && ring.first().map(|corner| corner.0) == ring.last().map(|corner| corner.0)
        {
            ring.pop();
        }
        let mut distinct = BTreeSet::new();
        for (index, (vertex, _)) in ring.iter().enumerate() {
            if index % 1024 == 0 {
                check_cancelled(context)?;
            }
            distinct.insert(*vertex);
        }
        if ring.len() < 3 || distinct.len() < 3 || distinct.len() != ring.len() {
            return Err(GeometryError::PolyRefusal {
                code: "poly.weld_collapses_face",
                element: Some(Element::Face(face)),
            });
        }
        face_sizes.push(
            u32::try_from(ring.len())
                .map_err(|_| GeometryError::InvalidMesh("welded face size exceeds u32"))?,
        );
        for (index, (vertex, source_corner)) in ring.iter().enumerate() {
            if index % 1024 == 0 {
                check_cancelled(context)?;
            }
            corners.push(*vertex);
            corner_sources.push(*source_corner);
        }
        start = end;
    }
    Ok((face_sizes, corners, corner_sources))
}

pub(super) fn construct_candidate(
    mesh: &PolyMesh,
    representatives: &[usize],
    vertex_remap: &[u32],
    face_sizes: &[u32],
    corners: &[u32],
    corner_sources: &[usize],
    context: &ExecutionContext,
) -> Result<PolyMesh, GeometryError> {
    let mut positions = Vec::with_capacity(representatives.len());
    for (index, &representative) in representatives.iter().enumerate() {
        if index % 1024 == 0 {
            check_cancelled(context)?;
        }
        positions.push(mesh.positions()[representative]);
    }
    let mut candidate = PolyMesh::new(positions, face_sizes.to_vec(), corners.to_vec())?;
    let original_topology = Topology::build(mesh, context)?;
    let candidate_topology = Topology::build(&candidate, context)?;
    let edge_sources = edge_source_indices(
        &original_topology,
        &candidate_topology,
        vertex_remap,
        context,
    )?;
    let mut edge_keys = Vec::with_capacity(candidate_topology.edges().len());
    for (index, &key) in candidate_topology.edges().keys().enumerate() {
        if index % 1024 == 0 {
            check_cancelled(context)?;
        }
        edge_keys.push(key);
    }
    for (name, attribute) in mesh.attributes() {
        if context.is_cancelled() {
            return Err(GeometryError::Cancelled);
        }
        let data = match attribute.domain {
            Domain::Point => select_data(&attribute.data, representatives, context)?,
            Domain::Corner => select_data(&attribute.data, corner_sources, context)?,
            Domain::Edge => select_edge_data(&attribute.data, &edge_sources, &edge_keys, context)?,
            Domain::Face => attribute.data.clone(),
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
    Ok(candidate)
}

pub(super) fn validate_candidate(
    candidate: &PolyMesh,
    representatives: &[usize],
    before: &crate::ValidationReport,
    context: &ExecutionContext,
) -> Result<(), GeometryError> {
    let after = validate(candidate, context)?;
    let mut prior_nonmanifold = BTreeSet::new();
    for (index, edge) in before.nonmanifold_edges.iter().enumerate() {
        if index % 1024 == 0 {
            check_cancelled(context)?;
        }
        prior_nonmanifold.insert((edge.0, edge.1));
    }
    for (index, &(a, b, _)) in after.nonmanifold_edges.iter().enumerate() {
        if index % 1024 == 0 {
            check_cancelled(context)?;
        }
        let old_a = source_vertex(representatives, a)?;
        let old_b = source_vertex(representatives, b)?;
        if !prior_nonmanifold.contains(&(old_a.min(old_b), old_a.max(old_b))) {
            return Err(GeometryError::PolyRefusal {
                code: "poly.nonmanifold",
                element: Some(Element::Edge(a, b)),
            });
        }
    }
    let mut before_slivers = BTreeSet::new();
    for (index, &face) in before.degenerate_faces.iter().enumerate() {
        if index % 1024 == 0 {
            check_cancelled(context)?;
        }
        before_slivers.insert(face);
    }
    for (index, &face) in after.degenerate_faces.iter().enumerate() {
        if index % 1024 == 0 {
            check_cancelled(context)?;
        }
        if !before_slivers.contains(&face) {
            return Err(GeometryError::PolyRefusal {
                code: "poly.face_degenerate",
                element: Some(Element::Face(face)),
            });
        }
    }
    let mut prior_winding = BTreeSet::new();
    for (index, &edge) in before.inconsistent_winding_edges.iter().enumerate() {
        if index % 1024 == 0 {
            check_cancelled(context)?;
        }
        prior_winding.insert(edge);
    }
    for (index, &(a, b)) in after.inconsistent_winding_edges.iter().enumerate() {
        if index % 1024 == 0 {
            check_cancelled(context)?;
        }
        let old_a = source_vertex(representatives, a)?;
        let old_b = source_vertex(representatives, b)?;
        if !prior_winding.contains(&(old_a.min(old_b), old_a.max(old_b))) {
            return Err(GeometryError::PolyRefusal {
                code: "poly.winding_conflict",
                element: Some(Element::Edge(a, b)),
            });
        }
    }
    Ok(())
}

fn source_vertex(representatives: &[usize], vertex: u32) -> Result<u32, GeometryError> {
    let index = usize::try_from(vertex)
        .map_err(|_| GeometryError::InvalidMesh("vertex index overflows usize"))?;
    u32::try_from(
        *representatives
            .get(index)
            .ok_or(GeometryError::InvalidMesh(
                "candidate edge refers to missing vertex",
            ))?,
    )
    .map_err(|_| GeometryError::InvalidMesh("source vertex exceeds u32"))
}
