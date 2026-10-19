//! Conservative triangle self-intersection test for generated sweep meshes.
use crate::{Fail, budget};
use std::collections::{BTreeMap, HashSet};
use ve_kernel::{ExecutionContext, Mesh};

const MAX_CELL_REFERENCES: usize = 250_000;
const MAX_CANDIDATE_PAIRS: usize = 1_000_000;

pub(super) fn intersects(
    mesh: &Mesh,
    path_points: usize,
    profile_points: usize,
    closed: bool,
    context: &ExecutionContext,
) -> Result<Option<(usize, usize)>, Fail> {
    let triangles = mesh.indices.len() / 3;
    if triangles < 4 {
        return Ok(None);
    }
    let Some((minimum, _)) = bounds(mesh) else {
        return Ok(None);
    };
    // Cubic cells sized to the average triangle, so a long thin tube does not smear each triangle
    // over many cells. A sorted map keeps the first reported pair deterministic across targets.
    let size = cell_size(mesh, triangles);
    let mut cells = BTreeMap::<[i64; 3], Vec<usize>>::new();
    let mut references = 0_usize;
    for triangle in 0..triangles {
        if triangle % 256 == 0 && context.is_cancelled() {
            return Err(crate::fail(
                "eval.cancelled",
                "sweep intersection grid cancelled",
            ));
        }
        let points = tri(mesh, triangle);
        let (lo, hi) = tri_bounds(points);
        let low = [0, 1, 2].map(|axis| cell_coord(lo[axis], minimum[axis], size));
        let high = [0, 1, 2].map(|axis| cell_coord(hi[axis], minimum[axis], size));
        for x in low[0]..=high[0] {
            for y in low[1]..=high[1] {
                for z in low[2]..=high[2] {
                    references = references.saturating_add(1);
                    if references > MAX_CELL_REFERENCES {
                        return Err(budget::exceeded(
                            "budget.triangles",
                            MAX_CELL_REFERENCES,
                            references,
                        ));
                    }
                    cells.entry([x, y, z]).or_default().push(triangle);
                }
            }
        }
    }
    let mut pairs = HashSet::new();
    for cell in cells.values() {
        if context.is_cancelled() {
            return Err(crate::fail(
                "eval.cancelled",
                "sweep intersection cells cancelled",
            ));
        }
        for (offset, &first) in cell.iter().enumerate() {
            for &second in &cell[offset + 1..] {
                if adjacent(first, second, path_points, profile_points, closed)
                    || shares_vertex(mesh, first, second)
                {
                    continue;
                }
                let (first_points, second_points) = (tri(mesh, first), tri(mesh, second));
                // Only pairs whose boxes overlap count against the budget: a straight tube packs
                // many far-apart-in-sequence triangles into one cell, and those are rejected here.
                if !boxes_overlap(first_points, second_points) {
                    continue;
                }
                let pair = (first.min(second), first.max(second));
                if !pairs.insert(pair) {
                    continue;
                }
                if pairs.len() > MAX_CANDIDATE_PAIRS {
                    return Err(budget::exceeded(
                        "budget.triangles",
                        MAX_CANDIDATE_PAIRS,
                        pairs.len(),
                    ));
                }
                if triangle_intersects(first_points, second_points) {
                    return Ok(Some(pair));
                }
            }
        }
    }
    Ok(None)
}

/// The mean longest box side of a triangle, so each triangle touches only a few cells.
fn cell_size(mesh: &Mesh, triangles: usize) -> f64 {
    let total = (0..triangles)
        .map(|triangle| {
            let (lo, hi) = tri_bounds(tri(mesh, triangle));
            (0..3)
                .map(|axis| hi[axis] - lo[axis])
                .fold(0.0_f64, f64::max)
        })
        .sum::<f64>();
    (total / f64::from(u32::try_from(triangles).unwrap_or(u32::MAX))).max(1.0e-7)
}

fn adjacent(a: usize, b: usize, path: usize, profile: usize, closed: bool) -> bool {
    let side_triangles = path.saturating_sub(usize::from(!closed)) * profile * 2;
    let a_cap = a >= side_triangles;
    let b_cap = b >= side_triangles;
    if a_cap && b_cap {
        return true;
    }
    if a_cap || b_cap {
        let side = if a_cap { b } else { a };
        let segment = side / (profile * 2);
        return segment == 0 || (!closed && segment + 1 == path - 1);
    }
    let ar = a / (profile * 2);
    let br = b / (profile * 2);
    ar.abs_diff(br) <= 1 || (closed && (ar == 0 && br + 1 == path || br == 0 && ar + 1 == path))
}

fn shares_vertex(mesh: &Mesh, a: usize, b: usize) -> bool {
    let aa = &mesh.indices[a * 3..a * 3 + 3];
    let bb = &mesh.indices[b * 3..b * 3 + 3];
    aa.iter().any(|index| bb.contains(index))
}

fn tri(mesh: &Mesh, index: usize) -> [[f64; 3]; 3] {
    [0, 1, 2].map(|corner| {
        usize::try_from(mesh.indices[index * 3 + corner])
            .ok()
            .and_then(|i| mesh.positions.get(i))
            .map_or([0.0; 3], |p| p.map(f64::from))
    })
}

fn boxes_overlap(a: [[f64; 3]; 3], b: [[f64; 3]; 3]) -> bool {
    let (alo, ahi) = tri_bounds(a);
    let (blo, bhi) = tri_bounds(b);
    (0..3).all(|axis| ahi[axis] >= blo[axis] - 1.0e-7 && bhi[axis] >= alo[axis] - 1.0e-7)
}

fn triangle_intersects(a: [[f64; 3]; 3], b: [[f64; 3]; 3]) -> bool {
    if !boxes_overlap(a, b) {
        return false;
    }
    let na = cross(sub(a[1], a[0]), sub(a[2], a[0]));
    let nb = cross(sub(b[1], b[0]), sub(b[2], b[0]));
    let la = length(na);
    let lb = length(nb);
    if la < 1.0e-14 || lb < 1.0e-14 {
        return false;
    }
    let da = dot(na, sub(b[0], a[0]));
    let db = dot(nb, sub(a[0], b[0]));
    if da.abs() <= 1.0e-8 * la && db.abs() <= 1.0e-8 * lb {
        return coplanar_intersects(a, b, na);
    }
    edges_hit(a, b) || edges_hit(b, a)
}

fn edges_hit(edges: [[f64; 3]; 3], face: [[f64; 3]; 3]) -> bool {
    (0..3).any(|i| segment_triangle(edges[i], edges[(i + 1) % 3], face))
}

#[expect(
    clippy::many_single_char_names,
    reason = "Möller-Trumbore uses the standard compact barycentric notation."
)]
fn segment_triangle(start: [f64; 3], end: [f64; 3], t: [[f64; 3]; 3]) -> bool {
    let direction = sub(end, start);
    let e1 = sub(t[1], t[0]);
    let e2 = sub(t[2], t[0]);
    let p = cross(direction, e2);
    let det = dot(e1, p);
    if det.abs() < 1.0e-12 {
        return false;
    }
    let inv = 1.0 / det;
    let s = sub(start, t[0]);
    let u = dot(s, p) * inv;
    let q = cross(s, e1);
    let v = dot(direction, q) * inv;
    let at = dot(e2, q) * inv;
    u >= -1.0e-9 && v >= -1.0e-9 && u + v <= 1.0 + 1.0e-9 && (-1.0e-9..=1.0 + 1.0e-9).contains(&at)
}

fn coplanar_intersects(a: [[f64; 3]; 3], b: [[f64; 3]; 3], normal: [f64; 3]) -> bool {
    let drop = (0..3)
        .max_by(|&x, &y| normal[x].abs().total_cmp(&normal[y].abs()))
        .unwrap_or(2);
    let axes = match drop {
        0 => [1, 2],
        1 => [0, 2],
        _ => [0, 1],
    };
    for (x, y) in [(a, b), (b, a)] {
        for i in 0..3 {
            if segments_2d(x[i], x[(i + 1) % 3], y[0], y[1], axes)
                || segments_2d(x[i], x[(i + 1) % 3], y[1], y[2], axes)
                || segments_2d(x[i], x[(i + 1) % 3], y[2], y[0], axes)
            {
                return true;
            }
        }
        if inside_2d(x[0], y, axes) {
            return true;
        }
    }
    false
}

fn segments_2d(a: [f64; 3], b: [f64; 3], c: [f64; 3], d: [f64; 3], axes: [usize; 2]) -> bool {
    let orient = |p: [f64; 3], q: [f64; 3], r: [f64; 3]| {
        (q[axes[0]] - p[axes[0]]) * (r[axes[1]] - p[axes[1]])
            - (q[axes[1]] - p[axes[1]]) * (r[axes[0]] - p[axes[0]])
    };
    let (o1, o2, o3, o4) = (
        orient(a, b, c),
        orient(a, b, d),
        orient(c, d, a),
        orient(c, d, b),
    );
    let crosses = (o1 > 1.0e-12 && o2 < -1.0e-12 || o1 < -1.0e-12 && o2 > 1.0e-12)
        && (o3 > 1.0e-12 && o4 < -1.0e-12 || o3 < -1.0e-12 && o4 > 1.0e-12);
    crosses
        || (o1.abs() <= 1.0e-12 && on_segment(c, a, b, axes))
        || (o2.abs() <= 1.0e-12 && on_segment(d, a, b, axes))
        || (o3.abs() <= 1.0e-12 && on_segment(a, c, d, axes))
        || (o4.abs() <= 1.0e-12 && on_segment(b, c, d, axes))
}

fn on_segment(p: [f64; 3], a: [f64; 3], b: [f64; 3], axes: [usize; 2]) -> bool {
    (0..2).all(|i| {
        let coordinate = axes[i];
        p[coordinate] >= a[coordinate].min(b[coordinate]) - 1.0e-9
            && p[coordinate] <= a[coordinate].max(b[coordinate]) + 1.0e-9
    })
}

#[expect(
    clippy::many_single_char_names,
    reason = "2D orientation predicates use standard point labels."
)]
fn inside_2d(p: [f64; 3], t: [[f64; 3]; 3], axes: [usize; 2]) -> bool {
    let orient = |a: [f64; 3], b: [f64; 3]| {
        (b[axes[0]] - a[axes[0]]) * (p[axes[1]] - a[axes[1]])
            - (b[axes[1]] - a[axes[1]]) * (p[axes[0]] - a[axes[0]])
    };
    let (a, b, c) = (orient(t[0], t[1]), orient(t[1], t[2]), orient(t[2], t[0]));
    (a >= -1.0e-9 && b >= -1.0e-9 && c >= -1.0e-9) || (a <= 1.0e-9 && b <= 1.0e-9 && c <= 1.0e-9)
}

fn bounds(mesh: &Mesh) -> Option<([f32; 3], [f32; 3])> {
    let first = *mesh.positions.first()?;
    Some(
        mesh.positions
            .iter()
            .fold((first, first), |(mut lo, mut hi), p| {
                for axis in 0..3 {
                    lo[axis] = lo[axis].min(p[axis]);
                    hi[axis] = hi[axis].max(p[axis]);
                }
                (lo, hi)
            }),
    )
}

fn tri_bounds(t: [[f64; 3]; 3]) -> ([f64; 3], [f64; 3]) {
    let mut lo = t[0];
    let mut hi = t[0];
    for p in &t[1..] {
        for axis in 0..3 {
            lo[axis] = lo[axis].min(p[axis]);
            hi[axis] = hi[axis].max(p[axis]);
        }
    }
    (lo, hi)
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "Coordinates are bounded by the kernel extent, so the cell index fits an i64."
)]
fn cell_coord(value: f64, origin: f32, size: f64) -> i64 {
    ((value - f64::from(origin)) / size).floor() as i64
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn length(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}
