//! A deterministic 3D convex hull (quickhull) over a point cloud.

use std::collections::{HashMap, HashSet};

use glam::Vec3;

use crate::MAX_HULL_POINTS;
use crate::error::MeasureError;

/// A convex hull as a triangle mesh.
#[derive(Clone, Debug, PartialEq)]
pub struct Hull {
    positions: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

impl Hull {
    /// The hull's vertices.
    #[must_use]
    pub fn positions(&self) -> &[[f32; 3]] {
        &self.positions
    }

    /// The hull's triangle indices.
    #[must_use]
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    /// How many vertices the hull has.
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    /// How many triangles the hull has.
    #[must_use]
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// The axis-aligned box with the given corners, as a hull.
    pub(crate) fn box_hull(min: Vec3, max: Vec3) -> Self {
        let positions = vec![
            [min.x, min.y, min.z],
            [max.x, min.y, min.z],
            [max.x, max.y, min.z],
            [min.x, max.y, min.z],
            [min.x, min.y, max.z],
            [max.x, min.y, max.z],
            [max.x, max.y, max.z],
            [min.x, max.y, max.z],
        ];
        let indices = vec![
            0, 3, 2, 0, 2, 1, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 3, 7, 6, 3, 6, 2, 0, 4, 7, 0, 7,
            3, 1, 2, 6, 1, 6, 5,
        ];
        Self { positions, indices }
    }
}

/// The convex hull of a point cloud, as a closed triangle mesh.
///
/// Quickhull: start from the tetrahedron of extreme points, then repeatedly add the point furthest
/// outside a face, replacing the faces it can see with a cone to it. Refuses more than
/// [`MAX_HULL_POINTS`] points and returns [`MeasureError::Degenerate`] for collinear or coplanar
/// input rather than guessing a hull.
///
/// # Errors
/// Returns [`MeasureError`] when the point set is above the budget, has a non-finite coordinate, or
/// has no volume (fewer than four distinct points, collinear, or coplanar).
pub fn convex_hull(points: &[[f32; 3]]) -> Result<Hull, MeasureError> {
    if points.len() > MAX_HULL_POINTS {
        return Err(MeasureError::PointBudget {
            points: points.len(),
            limit: MAX_HULL_POINTS,
        });
    }
    for (i, p) in points.iter().enumerate() {
        if !p.iter().all(|c| c.is_finite()) {
            return Err(MeasureError::NonFinite(i));
        }
    }
    let pts = dedup(points);
    if pts.len() < 4 {
        return Err(MeasureError::Degenerate("fewer than four distinct points"));
    }
    let scale = diagonal(&pts);
    let eps = (scale * 1e-6).max(1e-9);
    let [i0, i1, i2, i3] = initial_tetra(&pts, eps)?;
    let interior =
        (pts[i0 as usize] + pts[i1 as usize] + pts[i2 as usize] + pts[i3 as usize]) / 4.0;
    let mut faces = vec![
        make_face(&pts, i0, i1, i2, interior, eps),
        make_face(&pts, i0, i3, i1, interior, eps),
        make_face(&pts, i0, i2, i3, interior, eps),
        make_face(&pts, i1, i3, i2, interior, eps),
    ];
    for i in 0..pts.len() as u32 {
        if [i0, i1, i2, i3].contains(&i) {
            continue;
        }
        for face in &mut faces {
            if face.normal.dot(pts[i as usize]) > face.offset + eps {
                face.outer.push(i);
                break;
            }
        }
    }
    expand(&pts, &mut faces, interior, eps);
    compact(&pts, &faces)
}

fn expand(pts: &[Vec3], faces: &mut Vec<Face>, interior: Vec3, eps: f32) {
    while let Some(index) = faces.iter().position(|f| !f.removed && !f.outer.is_empty()) {
        let apex = furthest(pts, &faces[index]);
        let point = pts[apex as usize];
        let visible: Vec<usize> = faces
            .iter()
            .enumerate()
            .filter(|(_, f)| !f.removed && f.normal.dot(point) > f.offset + eps)
            .map(|(i, _)| i)
            .collect();
        let horizon = horizon_edges(faces, &visible);
        let mut pool = Vec::new();
        for &vi in &visible {
            pool.append(&mut faces[vi].outer);
            faces[vi].removed = true;
        }
        let first_new = faces.len();
        for (a, b) in horizon {
            faces.push(make_face(pts, a, b, apex, interior, eps));
        }
        for &i in &pool {
            for face in &mut faces[first_new..] {
                if face.normal.dot(pts[i as usize]) > face.offset + eps {
                    face.outer.push(i);
                    break;
                }
            }
        }
    }
}

fn horizon_edges(faces: &[Face], visible: &[usize]) -> Vec<(u32, u32)> {
    let mut directed = Vec::new();
    for &vi in visible {
        let f = &faces[vi];
        directed.push((f.a, f.b));
        directed.push((f.b, f.c));
        directed.push((f.c, f.a));
    }
    let present: HashSet<(u32, u32)> = directed.iter().copied().collect();
    directed
        .iter()
        .copied()
        .filter(|&(a, b)| !present.contains(&(b, a)))
        .collect()
}

fn furthest(pts: &[Vec3], face: &Face) -> u32 {
    let mut apex = face.outer[0];
    let mut best = face.normal.dot(pts[apex as usize]) - face.offset;
    for &i in &face.outer {
        let distance = face.normal.dot(pts[i as usize]) - face.offset;
        if distance > best {
            best = distance;
            apex = i;
        }
    }
    apex
}

/// A hull face: a triangle, its outward plane, and the points outside it.
struct Face {
    a: u32,
    b: u32,
    c: u32,
    normal: Vec3,
    offset: f32,
    outer: Vec<u32>,
    removed: bool,
}

fn make_face(pts: &[Vec3], a: u32, b: u32, c: u32, interior: Vec3, eps: f32) -> Face {
    let (a, mut b, mut c) = (a, b, c);
    let mut normal = (pts[b as usize] - pts[a as usize]).cross(pts[c as usize] - pts[a as usize]);
    let mut offset = normal.dot(pts[a as usize]);
    if normal.dot(interior) > offset {
        std::mem::swap(&mut b, &mut c);
        normal = -normal;
        offset = -offset;
    }
    let _ = eps;
    Face {
        a,
        b,
        c,
        normal,
        offset,
        outer: Vec::new(),
        removed: false,
    }
}

fn compact(pts: &[Vec3], faces: &[Face]) -> Result<Hull, MeasureError> {
    let mut map: HashMap<u32, u32> = HashMap::new();
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for face in faces.iter().filter(|f| !f.removed) {
        for vertex in [face.a, face.b, face.c] {
            let id = *map.entry(vertex).or_insert_with(|| {
                positions.push(pts[vertex as usize].to_array());
                (positions.len() - 1) as u32
            });
            indices.push(id);
        }
    }
    if positions.len() < 4 || indices.len() < 12 {
        return Err(MeasureError::Degenerate("the hull has no volume"));
    }
    Ok(Hull { positions, indices })
}

fn initial_tetra(pts: &[Vec3], eps: f32) -> Result<[u32; 4], MeasureError> {
    let mut candidates: Vec<u32> = Vec::new();
    for axis in 0..3 {
        let mut min_i = 0usize;
        let mut max_i = 0usize;
        for (i, p) in pts.iter().enumerate() {
            if p.to_array()[axis] < pts[min_i].to_array()[axis] {
                min_i = i;
            }
            if p.to_array()[axis] > pts[max_i].to_array()[axis] {
                max_i = i;
            }
        }
        candidates.push(min_i as u32);
        candidates.push(max_i as u32);
    }
    let mut i0 = 0;
    let mut i1 = 0;
    let mut span = -1.0f32;
    for &x in &candidates {
        for &y in &candidates {
            let distance = pts[x as usize].distance(pts[y as usize]);
            if distance > span {
                span = distance;
                i0 = x;
                i1 = y;
            }
        }
    }
    if span <= eps {
        return Err(MeasureError::Degenerate("all points coincide"));
    }
    let mut i2 = u32::MAX;
    let mut best = 0.0f32;
    for (i, p) in pts.iter().enumerate() {
        let distance = distance_to_line(*p, pts[i0 as usize], pts[i1 as usize]);
        if distance > best {
            best = distance;
            i2 = i as u32;
        }
    }
    if best <= eps {
        return Err(MeasureError::Degenerate("points are collinear"));
    }
    let normal = (pts[i1 as usize] - pts[i0 as usize]).cross(pts[i2 as usize] - pts[i0 as usize]);
    let mut i3 = u32::MAX;
    let mut best = 0.0f32;
    for (i, p) in pts.iter().enumerate() {
        let distance = normal.dot(*p - pts[i0 as usize]).abs() / normal.length();
        if distance > best {
            best = distance;
            i3 = i as u32;
        }
    }
    if best <= eps {
        return Err(MeasureError::Degenerate("points are coplanar"));
    }
    Ok([i0, i1, i2, i3])
}

fn distance_to_line(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    ab.cross(p - a).length() / ab.length()
}

fn diagonal(pts: &[Vec3]) -> f32 {
    let mut min = pts[0];
    let mut max = pts[0];
    for p in pts {
        min = min.min(*p);
        max = max.max(*p);
    }
    (max - min).length()
}

fn dedup(points: &[[f32; 3]]) -> Vec<Vec3> {
    let mut seen: HashSet<[u32; 3]> = HashSet::new();
    let mut out = Vec::new();
    for p in points {
        let key = p.map(|c| {
            if c.to_bits() == 0x8000_0000 {
                0.0f32.to_bits()
            } else {
                c.to_bits()
            }
        });
        if seen.insert(key) {
            out.push(Vec3::from_array(*p));
        }
    }
    out
}
