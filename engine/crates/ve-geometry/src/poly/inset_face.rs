//! Inset one planar, strictly convex polygon face by a constant distance.

use crate::topology::check_cancelled;
use crate::{Domain, Element, GeometryError, PolyMesh, validate};
use ve_kernel::ExecutionContext;

/// Replaces one selected face by an inset cap and a ring of quads.
///
/// Edge-domain attributes are refused because the operation introduces edges with no defined
/// source values. Point and corner values are copied from corresponding source corners.
///
/// # Errors
/// Returns typed refusals for unsupported selection or geometry, structural errors, or cancellation.
pub fn inset_face(
    mesh: &PolyMesh,
    face: usize,
    distance: f32,
    context: &ExecutionContext,
) -> Result<PolyMesh, GeometryError> {
    let before = validate(mesh, context)?;
    if face >= mesh.face_count() {
        return Err(refusal("poly.selection_invalid", face));
    }
    if before.degenerate_faces.contains(&face) {
        return Err(refusal("poly.face_degenerate", face));
    }
    if !distance.is_finite() || distance <= 0.0 {
        return Err(refusal("poly.inset_distance_invalid", face));
    }
    if mesh
        .attributes()
        .values()
        .any(|attr| attr.domain == Domain::Edge)
    {
        return Err(refusal("poly.attribute_conflict", face));
    }
    let range = face_range(mesh, face)?;
    let source = &mesh.corners()[range.clone()];
    let points: Vec<[f64; 2]> = project(mesh, source, face)?;
    let inner = inset_polygon(&points, f64::from(distance), face)?;
    let (mut result, face_sources, corner_sources, point_sources) =
        make_candidate(mesh, face, &range, source, &inner, context)?;
    super::attrs::transfer(
        mesh,
        &mut result,
        &super::attrs::Sources {
            points: point_sources,
            faces: face_sources,
            corners: corner_sources,
        },
        context,
    )?;
    let after = validate(&result, context)?;
    if after.euler_characteristic != before.euler_characteristic
        || after.components != before.components
        || after.nonmanifold_edges.len() > before.nonmanifold_edges.len()
        || after.inconsistent_winding_edges.len() > before.inconsistent_winding_edges.len()
        || after.degenerate_faces.len() > before.degenerate_faces.len()
    {
        return Err(refusal("poly.topology_unexpected", face));
    }
    // The ring and the inner cap replace the face in place (`range.len()` ring quads, then the cap) and
    // must not cross the rest of the mesh.
    let new_faces: Vec<usize> = (face..=face + range.len()).collect();
    super::overlap::refuse_crossings(&result, &new_faces, context)?;
    Ok(result)
}

fn face_range(mesh: &PolyMesh, face: usize) -> Result<std::ops::Range<usize>, GeometryError> {
    let start = mesh.face_sizes()[..face]
        .iter()
        .try_fold(0_usize, |sum, size| {
            sum.checked_add(*size as usize)
                .ok_or(GeometryError::InvalidMesh("face range overflow"))
        })?;
    let end = start
        .checked_add(mesh.face_sizes()[face] as usize)
        .ok_or(GeometryError::InvalidMesh("face range overflow"))?;
    Ok(start..end)
}

fn project(mesh: &PolyMesh, corners: &[u32], face: usize) -> Result<Vec<[f64; 2]>, GeometryError> {
    let origin = position(mesh, corners[0]);
    let first = position(mesh, corners[1]);
    let u = normalize(sub(first, origin)).ok_or_else(|| refusal("poly.face_degenerate", face))?;
    let mut normal = [0.0; 3];
    for index in 0..corners.len() {
        let a = position(mesh, corners[index]);
        let b = position(mesh, corners[(index + 1) % corners.len()]);
        normal = add(normal, cross(sub(a, origin), sub(b, origin)));
    }
    let normal = normalize(normal).ok_or_else(|| refusal("poly.face_degenerate", face))?;
    let v = normalize(cross(normal, u)).ok_or_else(|| refusal("poly.face_degenerate", face))?;
    let mut points = Vec::with_capacity(corners.len());
    for &vertex in corners {
        let delta = sub(position(mesh, vertex), origin);
        if dot(delta, normal).abs() > 1.0e-5 {
            return Err(refusal("poly.face_nonplanar", face));
        }
        points.push([dot(delta, u), dot(delta, v)]);
    }
    Ok(points)
}

fn inset_polygon(
    points: &[[f64; 2]],
    distance: f64,
    face: usize,
) -> Result<Vec<[f64; 2]>, GeometryError> {
    let area = signed_area(points);
    if area.abs() <= 1.0e-12 {
        return Err(refusal("poly.face_degenerate", face));
    }
    let orientation = area.signum();
    let mut inward = Vec::with_capacity(points.len());
    for index in 0..points.len() {
        let edge = sub2(points[(index + 1) % points.len()], points[index]);
        let length = dot2(edge, edge).sqrt();
        if length <= 1.0e-12 {
            return Err(refusal("poly.face_degenerate", face));
        }
        inward.push([
            -edge[1] / length * orientation,
            edge[0] / length * orientation,
        ]);
    }
    if !is_strictly_convex(points, orientation) {
        return Err(refusal("poly.face_nonconvex", face));
    }
    let mut output = Vec::with_capacity(points.len());
    for index in 0..points.len() {
        let previous = (index + points.len() - 1) % points.len();
        let a = add2(points[index], scale2(inward[previous], distance));
        let b = add2(points[index], scale2(inward[index], distance));
        let p = intersect_shifted_edges(
            points[previous],
            points[index],
            points[index],
            points[(index + 1) % points.len()],
            a,
            b,
        )
        .ok_or_else(|| refusal("poly.inset_too_large", face))?;
        output.push(p);
    }
    if signed_area(&output).abs() <= area.abs() * 1.0e-10
        || !is_strictly_convex(&output, orientation)
        || !inside_convex(points, &output, orientation, distance)
    {
        return Err(refusal("poly.inset_too_large", face));
    }
    Ok(output)
}

fn intersect_shifted_edges(
    a0: [f64; 2],
    a1: [f64; 2],
    b0: [f64; 2],
    b1: [f64; 2],
    shifted_a: [f64; 2],
    shifted_b: [f64; 2],
) -> Option<[f64; 2]> {
    let direction_a = sub2(a1, a0);
    let direction_b = sub2(b1, b0);
    let denominator = cross2(direction_a, direction_b);
    if denominator.abs() <= 1.0e-12 {
        return None;
    }
    let distance = cross2(sub2(shifted_b, shifted_a), direction_b) / denominator;
    Some(add2(shifted_a, scale2(direction_a, distance)))
}

fn make_candidate(
    mesh: &PolyMesh,
    selected: usize,
    range: &std::ops::Range<usize>,
    source: &[u32],
    inner: &[[f64; 2]],
    context: &ExecutionContext,
) -> Result<(PolyMesh, Vec<usize>, Vec<usize>, Vec<usize>), GeometryError> {
    let origin = position(mesh, source[0]);
    let p1 = position(mesh, source[1]);
    let u = normalize(sub(p1, origin)).ok_or_else(|| refusal("poly.face_degenerate", selected))?;
    let normal = normalize(face_normal(mesh, source))
        .ok_or_else(|| refusal("poly.face_degenerate", selected))?;
    let v = normalize(cross(normal, u)).ok_or_else(|| refusal("poly.face_degenerate", selected))?;
    let mut positions = mesh.positions().to_vec();
    let mut new_points = Vec::new();
    for (index, point) in inner.iter().enumerate() {
        if index % 256 == 0 {
            check_cancelled(context)?;
        }
        let mapped = add(origin, add(scale(u, point[0]), scale(v, point[1])));
        positions.push([
            to_mesh_scalar(mapped[0])?,
            to_mesh_scalar(mapped[1])?,
            to_mesh_scalar(mapped[2])?,
        ]);
        new_points.push(
            u32::try_from(positions.len() - 1)
                .map_err(|_| GeometryError::InvalidMesh("vertex index exceeds u32"))?,
        );
    }
    let mut sizes = Vec::new();
    let mut corners = Vec::new();
    let mut face_sources = Vec::new();
    let mut corner_sources = Vec::new();
    let mut start = 0_usize;
    for (face, &size) in mesh.face_sizes().iter().enumerate() {
        if face == selected {
            for i in 0..source.len() {
                let next = (i + 1) % source.len();
                sizes.push(4);
                corners.extend([source[i], source[next], new_points[next], new_points[i]]);
                face_sources.push(face);
                corner_sources.extend([
                    range.start + i,
                    range.start + next,
                    range.start + next,
                    range.start + i,
                ]);
            }
            sizes.push(size);
            corners.extend_from_slice(&new_points);
            face_sources.push(face);
            corner_sources.extend(range.clone());
        } else {
            sizes.push(size);
            corners.extend_from_slice(&mesh.corners()[start..start + size as usize]);
            face_sources.push(face);
            corner_sources.extend(start..start + size as usize);
        }
        start = start
            .checked_add(size as usize)
            .ok_or(GeometryError::InvalidMesh("face range overflow"))?;
    }
    let mut point_sources: Vec<usize> = (0..mesh.positions().len()).collect();
    point_sources.extend(source.iter().map(|vertex| *vertex as usize));
    Ok((
        PolyMesh::new(positions, sizes, corners)?,
        face_sources,
        corner_sources,
        point_sources,
    ))
}

fn face_normal(mesh: &PolyMesh, corners: &[u32]) -> [f64; 3] {
    let origin = position(mesh, corners[0]);
    let mut normal = [0.0; 3];
    for i in 0..corners.len() {
        normal = add(
            normal,
            cross(
                sub(position(mesh, corners[i]), origin),
                sub(position(mesh, corners[(i + 1) % corners.len()]), origin),
            ),
        );
    }
    normal
}
fn is_strictly_convex(points: &[[f64; 2]], orientation: f64) -> bool {
    (0..points.len()).all(|i| {
        cross2(
            sub2(points[(i + 1) % points.len()], points[i]),
            sub2(
                points[(i + 2) % points.len()],
                points[(i + 1) % points.len()],
            ),
        ) * orientation
            > 1.0e-10
    })
}
fn inside_convex(
    points: &[[f64; 2]],
    inner: &[[f64; 2]],
    orientation: f64,
    clearance: f64,
) -> bool {
    (0..points.len()).all(|edge| {
        let direction = sub2(points[(edge + 1) % points.len()], points[edge]);
        let edge_length = dot2(direction, direction).sqrt();
        inner.iter().all(|point| {
            cross2(direction, sub2(*point, points[edge])) * orientation
                >= (clearance - 1.0e-9) * edge_length
        })
    })
}
fn signed_area(points: &[[f64; 2]]) -> f64 {
    (0..points.len())
        .map(|i| cross2(points[i], points[(i + 1) % points.len()]))
        .sum::<f64>()
        * 0.5
}
fn position(mesh: &PolyMesh, index: u32) -> [f64; 3] {
    mesh.positions()[index as usize].map(f64::from)
}
#[expect(
    clippy::cast_possible_truncation,
    reason = "polygon positions are f32 by contract; inset math uses f64 then rounds to that storage precision"
)]
fn to_mesh_scalar(value: f64) -> Result<f32, GeometryError> {
    if !value.is_finite() || value.abs() > f64::from(f32::MAX) {
        return Err(GeometryError::InvalidMesh(
            "inset position exceeds f32 range",
        ));
    }
    Ok(value as f32)
}
fn normalize(a: [f64; 3]) -> Option<[f64; 3]> {
    let n = dot(a, a).sqrt();
    (n > 1.0e-12).then(|| scale(a, 1.0 / n))
}
fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn scale(a: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn add2(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] + b[0], a[1] + b[1]]
}
fn sub2(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
fn scale2(a: [f64; 2], s: f64) -> [f64; 2] {
    [a[0] * s, a[1] * s]
}
fn cross2(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
fn dot2(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn refusal(code: &'static str, face: usize) -> GeometryError {
    GeometryError::PolyRefusal {
        code,
        element: Some(Element::Face(face)),
    }
}
