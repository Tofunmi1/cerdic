//! Bridge two boundary loops with equal vertex counts.

use std::collections::BTreeMap;

use ve_kernel::ExecutionContext;

use crate::topology::check_cancelled;
use crate::{AttrData, Attribute, Domain, Element, GeometryError, PolyMesh, Topology, validate};

const MAX_ALIGNMENT_PAIRS: usize = 1_000_000;

/// Joins two extracted boundary loops with quads using deterministic minimum-distance alignment.
///
/// # Errors
/// Refuses invalid loop indices, mismatched loops, edge attributes, and invalid output topology;
/// also returns structural mesh errors or cancellation.
pub fn bridge_loops(
    mesh: &PolyMesh,
    first_loop: usize,
    second_loop: usize,
    context: &ExecutionContext,
) -> Result<PolyMesh, GeometryError> {
    let before = validate(mesh, context)?;
    let topology = Topology::build(mesh, context)?;
    let loops = topology.boundary_loops();
    if first_loop >= loops.len() || second_loop >= loops.len() || first_loop == second_loop {
        return Err(GeometryError::PolyRefusal {
            code: "poly.loop_not_boundary",
            element: None,
        });
    }
    let first = &loops[first_loop];
    let second = &loops[second_loop];
    if first.len() != second.len() {
        return Err(GeometryError::PolyRefusal {
            code: "poly.loop_mismatch",
            element: first.first().copied().map(Element::Vertex),
        });
    }
    if first
        .len()
        .checked_mul(first.len())
        .is_none_or(|work| work > MAX_ALIGNMENT_PAIRS)
    {
        return Err(GeometryError::PolyRefusal {
            code: "poly.bridge_pair_budget",
            element: first.first().copied().map(Element::Vertex),
        });
    }
    if mesh
        .attributes()
        .values()
        .any(|attribute| attribute.domain == Domain::Edge)
    {
        return Err(GeometryError::PolyRefusal {
            code: "poly.attribute_conflict",
            element: None,
        });
    }
    if let Some(&face) = before.degenerate_faces.first() {
        return Err(GeometryError::PolyRefusal {
            code: "poly.face_degenerate",
            element: Some(Element::Face(face)),
        });
    }
    let sources = boundary_sources(mesh, &topology, context)?;
    let (aligned, start) = align(mesh, first, second, context)?;
    build_candidate(
        mesh, &topology, first, &aligned, start, &sources, &before, context,
    )
}

fn boundary_sources(
    mesh: &PolyMesh,
    topology: &Topology,
    context: &ExecutionContext,
) -> Result<BTreeMap<(u32, u32), (usize, usize)>, GeometryError> {
    let mut output = BTreeMap::new();
    let mut start = 0_usize;
    for (face, &size) in mesh.face_sizes().iter().enumerate() {
        check_cancelled(context)?;
        let end = start
            .checked_add(size as usize)
            .ok_or(GeometryError::InvalidMesh("face range overflow"))?;
        for corner in start..end {
            let next = if corner + 1 == end { start } else { corner + 1 };
            let a = mesh.corners()[corner];
            let b = mesh.corners()[next];
            let key = (a.min(b), a.max(b));
            if topology.edges()[&key].face_count() == 1 {
                output.insert((a, b), (face, corner));
            }
        }
        start = end;
    }
    Ok(output)
}

fn align(
    mesh: &PolyMesh,
    first: &[u32],
    second: &[u32],
    context: &ExecutionContext,
) -> Result<(Vec<u32>, usize), GeometryError> {
    let count = first.len();
    let mut best: Option<(f64, usize, Vec<u32>)> = None;
    for start in 0..count {
        check_cancelled(context)?;
        let candidate: Vec<u32> = (0..count)
            .map(|i| second[(start + count - i) % count])
            .collect();
        let cost = candidate
            .iter()
            .zip(first)
            .try_fold(0.0_f64, |sum, (&a, &b)| {
                let pa = mesh.positions()[a as usize].map(f64::from);
                let pb = mesh.positions()[b as usize].map(f64::from);
                let dx = pa[0] - pb[0];
                let dy = pa[1] - pb[1];
                let dz = pa[2] - pb[2];
                let next = sum + dx * dx + dy * dy + dz * dz;
                next.is_finite().then_some(next)
            })
            .ok_or(GeometryError::InvalidMesh("bridge alignment cost overflow"))?;
        if best.as_ref().is_none_or(|(best_cost, best_start, _)| {
            cost.total_cmp(best_cost).is_lt()
                || (cost.total_cmp(best_cost).is_eq() && start < *best_start)
        }) {
            best = Some((cost, start, candidate));
        }
    }
    let (_, start, vertices) = best.ok_or(GeometryError::InvalidMesh("bridge loops are empty"))?;
    Ok((vertices, start))
}

fn build_candidate(
    mesh: &PolyMesh,
    topology: &Topology,
    first: &[u32],
    second: &[u32],
    _start: usize,
    sources: &BTreeMap<(u32, u32), (usize, usize)>,
    before: &crate::ValidationReport,
    context: &ExecutionContext,
) -> Result<PolyMesh, GeometryError> {
    let mut sizes = mesh.face_sizes().to_vec();
    let mut corners = mesh.corners().to_vec();
    let mut face_sources: Vec<usize> = (0..mesh.face_count()).collect();
    let mut corner_sources: Vec<usize> = (0..mesh.corners().len()).collect();
    for index in 0..first.len() {
        check_cancelled(context)?;
        let next = (index + 1) % first.len();
        let a = first[index];
        let a_next = first[next];
        let b = second[index];
        let b_next = second[next];
        sizes.push(4);
        corners.extend([a_next, a, b, b_next]);
        let &(face_a, corner_a) = sources
            .get(&(a, a_next))
            .ok_or_else(|| loop_error(first[index]))?;
        let &(face_b, corner_b) = sources
            .get(&(b_next, b))
            .ok_or_else(|| loop_error(second[index]))?;
        let a_end_corner = next_corner(mesh, face_a, corner_a)?;
        let b_end_corner = next_corner(mesh, face_b, corner_b)?;
        face_sources.push(face_a);
        corner_sources.extend([a_end_corner, corner_a, b_end_corner, corner_b]);
    }
    let mut candidate = PolyMesh::new(mesh.positions().to_vec(), sizes, corners)?;
    copy_attributes(
        mesh,
        &mut candidate,
        &face_sources,
        &corner_sources,
        context,
    )?;
    let after = validate(&candidate, context)?;
    if after.euler_characteristic != before.euler_characteristic
        || after.boundary_loops.saturating_add(2) != before.boundary_loops
        || after.nonmanifold_edges.len() > before.nonmanifold_edges.len()
        || after.inconsistent_winding_edges.len() > before.inconsistent_winding_edges.len()
        || after.degenerate_faces.len() > before.degenerate_faces.len()
        || after.components > before.components
    {
        return Err(GeometryError::PolyRefusal {
            code: "poly.topology_unexpected",
            element: first.first().copied().map(Element::Vertex),
        });
    }
    let _ = topology;
    // The bridge strip must not pass through the rest of the mesh.
    let new_faces: Vec<usize> = (mesh.face_count()..candidate.face_count()).collect();
    super::overlap::refuse_crossings(&candidate, &new_faces, context)?;
    Ok(candidate)
}

fn next_corner(mesh: &PolyMesh, face: usize, corner: usize) -> Result<usize, GeometryError> {
    let start = mesh.face_sizes()[..face]
        .iter()
        .try_fold(0_usize, |sum, size| {
            sum.checked_add(*size as usize)
                .ok_or(GeometryError::InvalidMesh("face range overflow"))
        })?;
    let end = start
        .checked_add(mesh.face_sizes()[face] as usize)
        .ok_or(GeometryError::InvalidMesh("face range overflow"))?;
    Ok(if corner + 1 == end { start } else { corner + 1 })
}

fn copy_attributes(
    mesh: &PolyMesh,
    candidate: &mut PolyMesh,
    faces: &[usize],
    corners: &[usize],
    context: &ExecutionContext,
) -> Result<(), GeometryError> {
    for (name, attribute) in mesh.attributes() {
        check_cancelled(context)?;
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
        AttrData::F32(v) => Ok(select!(F32, v)),
        AttrData::F64(v) => Ok(select!(F64, v)),
        AttrData::I32(v) => Ok(select!(I32, v)),
        AttrData::U32(v) => Ok(select!(U32, v)),
    }
}

fn loop_error(vertex: u32) -> GeometryError {
    GeometryError::PolyRefusal {
        code: "poly.loop_not_boundary",
        element: Some(Element::Vertex(vertex)),
    }
}
