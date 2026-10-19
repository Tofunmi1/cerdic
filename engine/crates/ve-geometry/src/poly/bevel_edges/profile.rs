//! Geometry of one beveled edge: the cut lines on its two faces and the rounded profile at each end.

use std::collections::BTreeMap;

use crate::{Element, GeometryError, PolyMesh, Topology};

pub(super) type Vec3 = [f64; 3];

/// One end of a beveled edge: which faces meet there, and the profile points that replace the vertex.
pub(super) struct End {
    pub(super) vertex: u32,
    /// The two faces of the beveled edge, and the third face at this valence-3 vertex.
    pub(super) faces: [usize; 3],
    /// Profile points from the first face's cut to the second face's cut.
    pub(super) profile: Vec<Vec3>,
    /// Whether the third face, read in its original winding, meets the first face's cut before the
    /// second's, so the profile runs along it in order rather than reversed. Decided on the input mesh
    /// because other bevels may already have rewritten the face's neighbours.
    pub(super) chain_in_order: bool,
}

pub(super) struct EdgePlan {
    pub(super) ends: [End; 2],
    /// Whether the first face traverses the edge from the first end to the second.
    pub(super) first_forward: bool,
}

pub(super) fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub(super) fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub(super) fn scale(a: Vec3, s: f64) -> Vec3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
pub(super) fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn normalize(a: Vec3) -> Option<Vec3> {
    let length = dot(a, a).sqrt();
    (length > 1.0e-12).then(|| scale(a, 1.0 / length))
}
pub(super) fn point(mesh: &PolyMesh, vertex: u32) -> Vec3 {
    mesh.positions()[vertex as usize].map(f64::from)
}

pub(super) fn refusal(code: &'static str, element: Element) -> GeometryError {
    GeometryError::PolyRefusal {
        code,
        element: Some(element),
    }
}

/// The corner vertices of a face.
pub(super) fn loop_of<'a>(
    mesh: &'a PolyMesh,
    ranges: &[std::ops::Range<usize>],
    face: usize,
) -> &'a [u32] {
    &mesh.corners()[ranges[face].clone()]
}

/// A planar face's unit normal (Newell), refusing a degenerate or warped face.
fn plane_normal(
    mesh: &PolyMesh,
    face_loop: &[u32],
    face: usize,
) -> Result<(Vec3, Vec3), GeometryError> {
    let points: Vec<Vec3> = face_loop.iter().map(|&v| point(mesh, v)).collect();
    let mut sum = [0.0; 3];
    let mut centre = [0.0; 3];
    for (i, a) in points.iter().enumerate() {
        let b = points[(i + 1) % points.len()];
        sum[0] += (a[1] - b[1]) * (a[2] + b[2]);
        sum[1] += (a[2] - b[2]) * (a[0] + b[0]);
        sum[2] += (a[0] - b[0]) * (a[1] + b[1]);
        centre = add(centre, *a);
    }
    let normal =
        normalize(sum).ok_or_else(|| refusal("poly.face_degenerate", Element::Face(face)))?;
    let centre = scale(
        centre,
        1.0 / f64::from(u32::try_from(points.len()).unwrap_or(1)),
    );
    let extent = points
        .iter()
        .map(|p| dot(sub(*p, centre), sub(*p, centre)).sqrt())
        .fold(0.0, f64::max);
    if points
        .iter()
        .any(|p| dot(sub(*p, centre), normal).abs() > extent * 1.0e-4)
    {
        return Err(refusal("poly.face_nonplanar", Element::Face(face)));
    }
    Ok((normal, centre))
}

/// The face loop's two neighbours of `vertex`: `(previous, next)`.
fn neighbours(face_loop: &[u32], vertex: u32) -> Option<(u32, u32)> {
    let at = face_loop.iter().position(|&v| v == vertex)?;
    let count = face_loop.len();
    Some((
        face_loop[(at + count - 1) % count],
        face_loop[(at + 1) % count],
    ))
}

/// The other neighbour of `vertex` in `face_loop`, besides `known`.
fn other_neighbour(face_loop: &[u32], vertex: u32, known: u32) -> Option<u32> {
    let (previous, next) = neighbours(face_loop, vertex)?;
    if previous == known {
        Some(next)
    } else if next == known {
        Some(previous)
    } else {
        None
    }
}

/// Plans the bevel of `edge`: validates its two ends and computes their profiles.
///
/// `consumed` accumulates, per undirected edge, the fraction cut away from it by bevel ends, so two
/// ends cutting the same edge cannot overlap.
pub(super) fn plan_edge(
    mesh: &PolyMesh,
    topology: &Topology,
    ranges: &[std::ops::Range<usize>],
    edge: (u32, u32),
    (width, segments): (f64, usize),
    consumed: &mut BTreeMap<(u32, u32), f64>,
) -> Result<EdgePlan, GeometryError> {
    let (u, v) = edge;
    let info = &topology.edges()[&(u.min(v), u.max(v))];
    let (first, second) = (info.uses()[0].face, info.uses()[1].face);
    let direction = normalize(sub(point(mesh, v), point(mesh, u)))
        .ok_or_else(|| refusal("poly.edge_degenerate", Element::Edge(u, v)))?;
    let side = |face: usize| -> Result<Vec3, GeometryError> {
        let (normal, centre) = plane_normal(mesh, loop_of(mesh, ranges, face), face)?;
        let across = normalize(cross(normal, direction))
            .ok_or_else(|| refusal("poly.face_degenerate", Element::Face(face)))?;
        let towards = sub(centre, point(mesh, u));
        Ok(if dot(across, towards) >= 0.0 {
            across
        } else {
            scale(across, -1.0)
        })
    };
    let (d1, d2) = (side(first)?, side(second)?);
    let cosine = dot(d1, d2).clamp(-1.0, 1.0);
    let angle = cosine.acos();
    if !(0.01..std::f64::consts::PI - 0.01).contains(&angle) {
        return Err(refusal("poly.bevel_flat_edge", Element::Edge(u, v)));
    }
    let first_loop = loop_of(mesh, ranges, first);
    let first_forward = neighbours(first_loop, u).is_some_and(|(_, next)| next == v);
    let make_end = |vertex: u32, other: u32, consumed: &mut BTreeMap<(u32, u32), f64>| {
        end_profile(
            mesh,
            topology,
            ranges,
            (vertex, other),
            [first, second],
            [d1, d2, direction],
            (width, segments, angle),
            consumed,
        )
    };
    let end_u = make_end(u, v, consumed)?;
    let end_v = make_end(v, u, consumed)?;
    Ok(EdgePlan {
        ends: [end_u, end_v],
        first_forward,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "the end profile needs the mesh context, the end, both faces and the profile numbers"
)]
fn end_profile(
    mesh: &PolyMesh,
    topology: &Topology,
    ranges: &[std::ops::Range<usize>],
    (vertex, other): (u32, u32),
    [first, second]: [usize; 2],
    [d1, d2, direction]: [Vec3; 3],
    (width, segments, angle): (f64, usize, f64),
    consumed: &mut BTreeMap<(u32, u32), f64>,
) -> Result<End, GeometryError> {
    let here = Element::Vertex(vertex);
    let faces = &topology.vertex_faces()[vertex as usize];
    if faces.len() != 3 {
        return Err(refusal("poly.bevel_vertex_unsupported", here));
    }
    let third = *faces
        .iter()
        .find(|&&f| f != first && f != second)
        .ok_or_else(|| refusal("poly.bevel_vertex_unsupported", here))?;
    let a1 = other_neighbour(loop_of(mesh, ranges, first), vertex, other)
        .ok_or_else(|| refusal("poly.bevel_vertex_unsupported", here))?;
    let a2 = other_neighbour(loop_of(mesh, ranges, second), vertex, other)
        .ok_or_else(|| refusal("poly.bevel_vertex_unsupported", here))?;
    let third_loop = loop_of(mesh, ranges, third);
    let ok = neighbours(third_loop, vertex)
        .is_some_and(|(p, n)| (p == a1 && n == a2) || (p == a2 && n == a1));
    if !ok {
        return Err(refusal("poly.bevel_vertex_unsupported", here));
    }
    let (n3, _) = plane_normal(mesh, third_loop, third)?;
    let x = point(mesh, vertex);
    for (edge_end, d) in [(a1, d1), (a2, d2)] {
        let run = dot(sub(point(mesh, edge_end), x), d);
        if run <= 1.0e-9 {
            return Err(refusal(
                "poly.bevel_too_large",
                Element::Edge(vertex, edge_end),
            ));
        }
        let key = (vertex.min(edge_end), vertex.max(edge_end));
        let used = consumed.entry(key).or_insert(0.0);
        *used += width / run;
        if *used > 0.9 {
            return Err(refusal(
                "poly.bevel_too_large",
                Element::Edge(vertex, edge_end),
            ));
        }
    }
    let run_along = dot(n3, direction);
    if run_along.abs() < 1.0e-6 {
        return Err(refusal("poly.bevel_vertex_unsupported", here));
    }
    let profile = arc_points(x, [d1, d2], (width, segments, angle))
        .into_iter()
        .map(|p| add(p, scale(direction, -dot(n3, sub(p, x)) / run_along)))
        .collect();
    let chain_in_order = neighbours(third_loop, vertex).is_some_and(|(previous, _)| previous == a1);
    Ok(End {
        vertex,
        faces: [first, second, third],
        profile,
        chain_in_order,
    })
}

/// Points of the circular arc tangent to both cut lines, from the first face's cut to the second's,
/// in the plane through `x` perpendicular to the edge.
fn arc_points(
    x: Vec3,
    [d1, d2]: [Vec3; 2],
    (width, segments, angle): (f64, usize, f64),
) -> Vec<Vec3> {
    let half = angle / 2.0;
    let bisector = normalize(add(d1, d2)).unwrap_or(d1);
    let centre = add(x, scale(bisector, width / half.cos()));
    let radius = width * half.tan();
    let start = scale(sub(add(x, scale(d1, width)), centre), 1.0 / radius);
    let end = scale(sub(add(x, scale(d2, width)), centre), 1.0 / radius);
    let theta = dot(start, end).clamp(-1.0, 1.0).acos();
    let count = u32::try_from(segments).unwrap_or(1);
    (0..=count)
        .map(|i| {
            let s = f64::from(i) / f64::from(count);
            let (w1, w2) = if theta.sin().abs() < 1.0e-9 {
                (1.0 - s, s)
            } else {
                (
                    ((1.0 - s) * theta).sin() / theta.sin(),
                    (s * theta).sin() / theta.sin(),
                )
            };
            add(centre, scale(add(scale(start, w1), scale(end, w2)), radius))
        })
        .collect()
}
