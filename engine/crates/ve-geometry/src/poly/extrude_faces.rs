//! Extrude one connected, planar face patch along a finite offset.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use ve_kernel::ExecutionContext;

use crate::topology::check_cancelled;
use crate::{Domain, Element, GeometryError, PolyMesh, Topology, validate};

/// Translates the selected planar patch and joins its boundary to the original mesh.
///
/// V1 accepts one connected patch and refuses edge-domain data because newly-created vertical
/// edges have no unambiguous source edge. Point, face, and corner data are copied from provenance.
///
/// # Errors
/// Returns a typed refusal for invalid selection, geometry, direction, or attributes, structural
/// mesh errors, or cancellation.
pub fn extrude_faces(
    mesh: &PolyMesh,
    selection: &[usize],
    offset: [f32; 3],
    context: &ExecutionContext,
) -> Result<PolyMesh, GeometryError> {
    let before = validate(mesh, context)?;
    let selected = selection_set(mesh.face_count(), selection)?;
    let topology = Topology::build(mesh, context)?;
    check_patch(&topology, &selected, &before, context)?;
    if mesh
        .attributes()
        .values()
        .any(|attr| attr.domain == Domain::Edge)
    {
        return Err(refusal(
            "poly.attribute_conflict",
            selected.first().copied(),
        ));
    }
    let ranges = ranges(mesh, context)?;
    let first_face = *selected
        .first()
        .ok_or_else(|| refusal("poly.selection_empty", None))?;
    let normal = patch_normal(mesh, &ranges, first_face, context)?;
    let offset64 = offset.map(f64::from);
    if offset64.iter().any(|v| !v.is_finite()) {
        return Err(refusal("poly.direction_invalid", selected.first().copied()));
    }
    let normal_offset = dot(normal, offset64);
    let offset_length = dot(offset64, offset64).sqrt();
    if offset_length <= 1.0e-8 || normal_offset <= offset_length * 1.0e-6 {
        return Err(refusal("poly.direction_invalid", selected.first().copied()));
    }
    check_planarity(mesh, &ranges, &selected, normal, context)?;
    let boundary = boundary_edges(mesh, &topology, &ranges, &selected, context)?;
    if boundary.is_empty() {
        return Err(refusal("poly.loop_invalid", selected.first().copied()));
    }
    build_result(
        mesh, &ranges, &selected, &boundary, offset, &before, context,
    )
}

fn selection_set(count: usize, selection: &[usize]) -> Result<BTreeSet<usize>, GeometryError> {
    let selected: BTreeSet<_> = selection.iter().copied().collect();
    if selected.is_empty() {
        return Err(refusal("poly.selection_empty", None));
    }
    if selected.len() != selection.len() || selected.iter().any(|&face| face >= count) {
        return Err(refusal("poly.selection_invalid", selected.first().copied()));
    }
    Ok(selected)
}

fn check_patch(
    topology: &Topology,
    selected: &BTreeSet<usize>,
    report: &crate::ValidationReport,
    context: &ExecutionContext,
) -> Result<(), GeometryError> {
    if let Some(&face) = selected
        .iter()
        .find(|face| report.degenerate_faces.contains(face))
    {
        return Err(refusal("poly.face_degenerate", Some(face)));
    }
    let first = *selected
        .first()
        .ok_or_else(|| refusal("poly.selection_empty", None))?;
    let mut seen = BTreeSet::from([first]);
    let mut queue = VecDeque::from([first]);
    while let Some(face) = queue.pop_front() {
        check_cancelled(context)?;
        for &neighbor in &topology.face_neighbors()[face] {
            if selected.contains(&neighbor) && seen.insert(neighbor) {
                queue.push_back(neighbor);
            }
        }
    }
    if seen.len() != selected.len() {
        return Err(refusal("poly.selection_disconnected", Some(first)));
    }
    for (&edge, info) in topology.edges() {
        if info
            .uses()
            .iter()
            .any(|usage| selected.contains(&usage.face))
        {
            if info.face_count() > 2 {
                return Err(GeometryError::PolyRefusal {
                    code: "poly.nonmanifold",
                    element: Some(Element::Edge(edge.0, edge.1)),
                });
            }
            if info.face_count() == 2 && info.uses()[0].forward == info.uses()[1].forward {
                return Err(GeometryError::PolyRefusal {
                    code: "poly.winding_conflict",
                    element: Some(Element::Edge(edge.0, edge.1)),
                });
            }
        }
    }
    Ok(())
}

fn ranges(
    mesh: &PolyMesh,
    context: &ExecutionContext,
) -> Result<Vec<std::ops::Range<usize>>, GeometryError> {
    let mut output = Vec::with_capacity(mesh.face_count());
    let mut start = 0_usize;
    for (index, &size) in mesh.face_sizes().iter().enumerate() {
        if index % 512 == 0 {
            check_cancelled(context)?;
        }
        let end = start
            .checked_add(size as usize)
            .ok_or(GeometryError::InvalidMesh("face range overflow"))?;
        output.push(start..end);
        start = end;
    }
    Ok(output)
}

fn patch_normal(
    mesh: &PolyMesh,
    ranges: &[std::ops::Range<usize>],
    face: usize,
    context: &ExecutionContext,
) -> Result<[f64; 3], GeometryError> {
    let vertices = &mesh.corners()[ranges[face].clone()];
    let mut n = [0.0; 3];
    for i in 0..vertices.len() {
        if i % 256 == 0 {
            check_cancelled(context)?;
        }
        let a = pos(mesh, vertices[i]);
        let b = pos(mesh, vertices[(i + 1) % vertices.len()]);
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let length = dot(n, n).sqrt();
    if length <= 1.0e-14 {
        return Err(refusal("poly.face_degenerate", Some(face)));
    }
    Ok(n.map(|value| value / length))
}

fn check_planarity(
    mesh: &PolyMesh,
    ranges: &[std::ops::Range<usize>],
    selected: &BTreeSet<usize>,
    normal: [f64; 3],
    context: &ExecutionContext,
) -> Result<(), GeometryError> {
    let first_face = *selected
        .first()
        .ok_or_else(|| refusal("poly.selection_empty", None))?;
    let first_corner = ranges[first_face].start;
    let origin = pos(mesh, mesh.corners()[first_corner]);
    let mut extent: f64 = 0.0;
    for &face in selected {
        for (i, &vertex) in mesh.corners()[ranges[face].clone()].iter().enumerate() {
            if i % 256 == 0 {
                check_cancelled(context)?;
            }
            let p = pos(mesh, vertex);
            extent = extent.max(
                dot(
                    [p[0] - origin[0], p[1] - origin[1], p[2] - origin[2]],
                    [p[0] - origin[0], p[1] - origin[1], p[2] - origin[2]],
                )
                .sqrt(),
            );
        }
    }
    let tolerance = extent.max(1.0) * 1.0e-5;
    for &face in selected {
        for &vertex in &mesh.corners()[ranges[face].clone()] {
            let p = pos(mesh, vertex);
            if dot(
                [p[0] - origin[0], p[1] - origin[1], p[2] - origin[2]],
                normal,
            )
            .abs()
                > tolerance
            {
                return Err(refusal("poly.patch_nonplanar", Some(face)));
            }
        }
    }
    Ok(())
}

fn boundary_edges(
    mesh: &PolyMesh,
    topology: &Topology,
    ranges: &[std::ops::Range<usize>],
    selected: &BTreeSet<usize>,
    context: &ExecutionContext,
) -> Result<Vec<(u32, u32, usize)>, GeometryError> {
    let mut edges = Vec::new();
    for &face in selected {
        check_cancelled(context)?;
        for corner in ranges[face].clone() {
            let next = if corner + 1 == ranges[face].end {
                ranges[face].start
            } else {
                corner + 1
            };
            let a = mesh.corners()[corner];
            let b = mesh.corners()[next];
            let key = (a.min(b), a.max(b));
            let selected_uses = topology.edges()[&key]
                .uses()
                .iter()
                .filter(|use_| selected.contains(&use_.face))
                .count();
            if selected_uses == 1 {
                edges.push((a, b, corner));
            }
        }
    }
    Ok(edges)
}

fn build_result(
    mesh: &PolyMesh,
    ranges: &[std::ops::Range<usize>],
    selected: &BTreeSet<usize>,
    boundary: &[(u32, u32, usize)],
    offset: [f32; 3],
    before: &crate::ValidationReport,
    context: &ExecutionContext,
) -> Result<PolyMesh, GeometryError> {
    let mut positions = mesh.positions().to_vec();
    let mut duplicate = BTreeMap::new();
    for &face in selected {
        for &vertex in &mesh.corners()[ranges[face].clone()] {
            let index = usize::try_from(vertex)
                .map_err(|_| GeometryError::InvalidMesh("vertex index overflow"))?;
            if let std::collections::btree_map::Entry::Vacant(entry) = duplicate.entry(vertex) {
                let new = u32::try_from(positions.len())
                    .map_err(|_| GeometryError::InvalidMesh("vertex index exceeds u32"))?;
                positions.push([
                    mesh.positions()[index][0] + offset[0],
                    mesh.positions()[index][1] + offset[1],
                    mesh.positions()[index][2] + offset[2],
                ]);
                entry.insert(new);
            }
        }
    }
    let mut sizes = mesh.face_sizes().to_vec();
    let mut corners = mesh.corners().to_vec();
    for &face in selected {
        for corner in ranges[face].clone() {
            corners[corner] = duplicate[&mesh.corners()[corner]];
        }
    }
    let mut face_sources: Vec<usize> = (0..mesh.face_count()).collect();
    let mut corner_sources: Vec<usize> = (0..mesh.corners().len()).collect();
    for &(a, b, source_corner) in boundary {
        sizes.push(4);
        corners.extend([a, b, duplicate[&b], duplicate[&a]]);
        let source_face = ranges
            .iter()
            .position(|range| range.contains(&source_corner))
            .ok_or(GeometryError::InvalidMesh("boundary source corner missing"))?;
        face_sources.push(source_face);
        let next_corner = if source_corner + 1 == ranges[source_face].end {
            ranges[source_face].start
        } else {
            source_corner + 1
        };
        corner_sources.extend([source_corner, next_corner, next_corner, source_corner]);
    }
    let mut candidate = PolyMesh::new(positions, sizes, corners)?;
    // Duplicated cap vertices copy the point data of the vertex they were made from.
    let mut point_sources: Vec<usize> = (0..mesh.positions().len()).collect();
    let mut duplicates: Vec<(u32, u32)> = duplicate.iter().map(|(&old, &new)| (new, old)).collect();
    duplicates.sort_unstable();
    point_sources.extend(duplicates.into_iter().map(|(_, old)| old as usize));
    super::attrs::transfer(
        mesh,
        &mut candidate,
        &super::attrs::Sources {
            points: point_sources,
            faces: face_sources,
            corners: corner_sources,
        },
        context,
    )?;
    let after = validate(&candidate, context)?;
    if after.euler_characteristic != before.euler_characteristic
        || after.components != before.components
        || after.nonmanifold_edges.len() > before.nonmanifold_edges.len()
        || after.inconsistent_winding_edges.len() > before.inconsistent_winding_edges.len()
        || after.degenerate_faces.len() > before.degenerate_faces.len()
    {
        return Err(refusal(
            "poly.topology_unexpected",
            selected.first().copied(),
        ));
    }
    // The moved cap and the new side faces must not pass through the rest of the mesh.
    let new_faces: Vec<usize> = selected
        .iter()
        .copied()
        .chain(mesh.face_count()..candidate.face_count())
        .collect();
    super::overlap::refuse_crossings(&candidate, &new_faces, context)?;
    Ok(candidate)
}

fn pos(mesh: &PolyMesh, index: u32) -> [f64; 3] {
    mesh.positions()[index as usize].map(f64::from)
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn refusal(code: &'static str, face: Option<usize>) -> GeometryError {
    GeometryError::PolyRefusal {
        code,
        element: face.map(Element::Face),
    }
}
