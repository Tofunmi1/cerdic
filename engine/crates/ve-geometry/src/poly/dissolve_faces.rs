//! Dissolve one connected, consistently wound face patch into a simple polygon.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use ve_kernel::ExecutionContext;

use crate::topology::check_cancelled;
use crate::{Element, GeometryError, PolyMesh, Topology, validate};

mod attributes;

/// Replaces a connected face patch by its single boundary polygon.
///
/// # Errors
/// Refuses disconnected patches, holes, pinches, winding conflicts, and conflicting face data.
pub fn dissolve_faces(
    mesh: &PolyMesh,
    selection: &[usize],
    context: &ExecutionContext,
) -> Result<PolyMesh, GeometryError> {
    let before = validate(mesh, context)?;
    let topology = Topology::build(mesh, context)?;
    let selected = selected_faces(mesh.face_count(), selection)?;
    check_connected_patch(&topology, &selected, context)?;
    attributes::check_patch_attributes(mesh, &selected)?;
    if let Some(face) = selected
        .iter()
        .find(|face| before.degenerate_faces.contains(face))
        .copied()
    {
        return Err(refusal("poly.face_degenerate", Some(face)));
    }
    let interior_edges = topology
        .edges()
        .values()
        .filter(|info| {
            info.face_count() == 2
                && info
                    .uses()
                    .iter()
                    .all(|usage| selected.contains(&usage.face))
        })
        .count();
    let ranges = face_ranges(mesh, context)?;
    let (boundary, corner_sources) = patch_boundary(mesh, &topology, &selected, &ranges, context)?;
    if boundary.len() < 3 {
        return Err(refusal(
            "poly.loop_invalid",
            selected.iter().next().copied(),
        ));
    }
    let (boundary, corner_sources) = order_boundary(
        boundary,
        corner_sources,
        selected.iter().next().copied(),
        context,
    )?;
    match crate::triangulate::validate_simple_loop(mesh, &boundary, context) {
        Ok(()) => {}
        Err(GeometryError::Cancelled) => return Err(GeometryError::Cancelled),
        Err(_) => {
            return Err(refusal(
                "poly.loop_invalid",
                selected.iter().next().copied(),
            ));
        }
    }
    let (mut candidate, face_sources, corner_sources, point_sources) = build_candidate(
        mesh,
        &ranges,
        &selected,
        &boundary,
        &corner_sources,
        context,
    )?;
    let candidate_topology = Topology::build(&candidate, context)?;
    attributes::propagate_attributes(
        mesh,
        &mut candidate,
        &topology,
        &candidate_topology,
        &face_sources,
        &corner_sources,
        &point_sources,
        context,
    )?;
    validate_candidate(
        &before,
        &candidate,
        &face_sources,
        &selected,
        interior_edges,
        context,
    )?;
    Ok(candidate)
}

fn validate_candidate(
    before: &crate::ValidationReport,
    candidate: &PolyMesh,
    face_sources: &[usize],
    selected: &BTreeSet<usize>,
    interior_edges: usize,
    context: &ExecutionContext,
) -> Result<(), GeometryError> {
    let after = validate(candidate, context)?;
    if before.euler_characteristic != after.euler_characteristic
        || before.components != after.components
        || after.edges + interior_edges != before.edges
        || after.faces + selected.len() != before.faces + 1
        || after.nonmanifold_edges.len() > before.nonmanifold_edges.len()
        || after.inconsistent_winding_edges.len() > before.inconsistent_winding_edges.len()
    {
        return Err(refusal(
            "poly.topology_unexpected",
            selected.iter().next().copied(),
        ));
    }
    let old_slivers: BTreeSet<_> = before.degenerate_faces.iter().copied().collect();
    for &face in &after.degenerate_faces {
        if face_sources
            .get(face)
            .is_some_and(|source_face| !old_slivers.contains(source_face))
        {
            return Err(refusal(
                "poly.face_degenerate",
                face_sources.get(face).copied(),
            ));
        }
    }
    Ok(())
}

fn selected_faces(
    face_count: usize,
    selection: &[usize],
) -> Result<BTreeSet<usize>, GeometryError> {
    if selection.is_empty() {
        return Err(refusal("poly.selection_empty", None));
    }
    let selected: BTreeSet<_> = selection.iter().copied().collect();
    if selected.len() != selection.len() || selected.iter().any(|&face| face >= face_count) {
        return Err(refusal(
            "poly.selection_invalid",
            selected.iter().next().copied(),
        ));
    }
    Ok(selected)
}

fn check_connected_patch(
    topology: &Topology,
    selected: &BTreeSet<usize>,
    context: &ExecutionContext,
) -> Result<(), GeometryError> {
    let Some(&first) = selected.iter().next() else {
        return Err(refusal("poly.selection_empty", None));
    };
    let mut visited = BTreeSet::from([first]);
    let mut queue = VecDeque::from([first]);
    while let Some(face) = queue.pop_front() {
        check_cancelled(context)?;
        for &neighbor in &topology.face_neighbors()[face] {
            if selected.contains(&neighbor) && visited.insert(neighbor) {
                queue.push_back(neighbor);
            }
        }
    }
    if visited.len() != selected.len() {
        return Err(refusal(
            "poly.selection_disconnected",
            selected.iter().next().copied(),
        ));
    }
    for (&edge, info) in topology.edges() {
        if info.face_count() > 2
            && info
                .uses()
                .iter()
                .any(|usage| selected.contains(&usage.face))
        {
            return Err(GeometryError::PolyRefusal {
                code: "poly.nonmanifold",
                element: Some(Element::Edge(edge.0, edge.1)),
            });
        }
    }
    Ok(())
}

fn face_ranges(
    mesh: &PolyMesh,
    context: &ExecutionContext,
) -> Result<Vec<std::ops::Range<usize>>, GeometryError> {
    let mut ranges = Vec::with_capacity(mesh.face_count());
    let mut start = 0_usize;
    for (face, &size) in mesh.face_sizes().iter().enumerate() {
        if face % 512 == 0 {
            check_cancelled(context)?;
        }
        let end = start
            .checked_add(
                usize::try_from(size)
                    .map_err(|_| GeometryError::InvalidMesh("face size overflow"))?,
            )
            .ok_or(GeometryError::InvalidMesh("face range overflow"))?;
        ranges.push(start..end);
        start = end;
    }
    Ok(ranges)
}

fn patch_boundary(
    mesh: &PolyMesh,
    topology: &Topology,
    selected: &BTreeSet<usize>,
    ranges: &[std::ops::Range<usize>],
    context: &ExecutionContext,
) -> Result<(Vec<(u32, u32, usize)>, Vec<usize>), GeometryError> {
    let mut boundary = Vec::new();
    for &face in selected {
        check_cancelled(context)?;
        let range = &ranges[face];
        for corner in range.clone() {
            let next = if corner + 1 == range.end {
                range.start
            } else {
                corner + 1
            };
            let a = mesh.corners()[corner];
            let b = mesh.corners()[next];
            let key = (a.min(b), a.max(b));
            let selected_count = topology.edges()[&key]
                .uses()
                .iter()
                .filter(|usage| selected.contains(&usage.face))
                .count();
            if selected_count == 2 {
                let uses = topology.edges()[&key].uses();
                if uses[0].forward == uses[1].forward {
                    return Err(GeometryError::PolyRefusal {
                        code: "poly.winding_conflict",
                        element: Some(Element::Edge(key.0, key.1)),
                    });
                }
            } else if selected_count == 1 {
                boundary.push((a, b, corner));
            } else {
                return Err(refusal("poly.nonmanifold", Some(face)));
            }
        }
    }
    Ok((boundary, Vec::new()))
}

fn order_boundary(
    edges: Vec<(u32, u32, usize)>,
    _sources: Vec<usize>,
    face: Option<usize>,
    context: &ExecutionContext,
) -> Result<(Vec<u32>, Vec<usize>), GeometryError> {
    let mut outgoing = BTreeMap::new();
    let mut incoming = BTreeSet::new();
    for (index, (a, b, corner)) in edges.iter().copied().enumerate() {
        if index % 256 == 0 {
            check_cancelled(context)?;
        }
        if outgoing.insert(a, (b, corner)).is_some() || !incoming.insert(b) {
            return Err(refusal("poly.loop_invalid", face));
        }
    }
    if outgoing.len() != incoming.len() || outgoing.keys().any(|vertex| !incoming.contains(vertex))
    {
        return Err(refusal("poly.loop_invalid", face));
    }
    let start = *outgoing
        .keys()
        .next()
        .ok_or_else(|| refusal("poly.loop_invalid", face))?;
    let mut vertices = Vec::with_capacity(edges.len());
    let mut corners = Vec::with_capacity(edges.len());
    let mut current = start;
    for _ in 0..edges.len() {
        check_cancelled(context)?;
        let &(next, corner) = outgoing
            .get(&current)
            .ok_or_else(|| refusal("poly.loop_invalid", face))?;
        vertices.push(current);
        corners.push(corner);
        current = next;
        if current == start {
            break;
        }
    }
    if current != start || vertices.len() != edges.len() {
        return Err(refusal("poly.loop_invalid", face));
    }
    Ok((vertices, corners))
}

fn build_candidate(
    mesh: &PolyMesh,
    ranges: &[std::ops::Range<usize>],
    selected: &BTreeSet<usize>,
    boundary: &[u32],
    boundary_corners: &[usize],
    context: &ExecutionContext,
) -> Result<(PolyMesh, Vec<usize>, Vec<usize>, Vec<usize>), GeometryError> {
    let Some(&first) = selected.iter().next() else {
        return Err(refusal("poly.selection_empty", None));
    };
    let mut sizes = Vec::new();
    let mut corners = Vec::new();
    let mut face_sources = Vec::new();
    let mut corner_sources = Vec::new();
    for (face, range) in ranges.iter().enumerate() {
        if face % 512 == 0 {
            check_cancelled(context)?;
        }
        if face == first {
            sizes.push(
                u32::try_from(boundary.len())
                    .map_err(|_| GeometryError::InvalidMesh("boundary too large"))?,
            );
            corners.extend_from_slice(boundary);
            face_sources.push(first);
            corner_sources.extend_from_slice(boundary_corners);
        } else if !selected.contains(&face) {
            sizes.push(
                u32::try_from(range.len())
                    .map_err(|_| GeometryError::InvalidMesh("face too large"))?,
            );
            corners.extend_from_slice(&mesh.corners()[range.clone()]);
            face_sources.push(face);
            corner_sources.extend(range.clone());
        }
    }
    let used: BTreeSet<usize> = corners
        .iter()
        .map(|vertex| usize::try_from(*vertex).unwrap_or(usize::MAX))
        .collect();
    let point_sources: Vec<usize> = used.into_iter().collect();
    let mut remap = vec![u32::MAX; mesh.positions().len()];
    let mut positions = Vec::with_capacity(point_sources.len());
    for (next, &source) in point_sources.iter().enumerate() {
        check_cancelled(context)?;
        remap[source] = u32::try_from(next)
            .map_err(|_| GeometryError::InvalidMesh("vertex index exceeds u32"))?;
        positions.push(mesh.positions()[source]);
    }
    for vertex in &mut corners {
        *vertex = *remap
            .get(
                usize::try_from(*vertex)
                    .map_err(|_| GeometryError::InvalidMesh("vertex index overflow"))?,
            )
            .ok_or(GeometryError::InvalidMesh("corner vertex missing"))?;
    }
    Ok((
        PolyMesh::new(positions, sizes, corners)?,
        face_sources,
        corner_sources,
        point_sources,
    ))
}

fn refusal(code: &'static str, face: Option<usize>) -> GeometryError {
    GeometryError::PolyRefusal {
        code,
        element: face.map(Element::Face),
    }
}
