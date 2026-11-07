//! A closed mesh as a truncated signed-distance grid.
//!
//! Voxels within `band` cells of the surface get the exact distance to the nearest triangle, signed by the pseudo-normal of the feature
//! (face, edge or vertex) the nearest point lies on, which is right even at sharp corners where a face normal alone is wrong. The rest of
//! the grid is outside or inside by a flood fill from the border, at plus or minus the truncation distance.

use std::collections::HashMap;

use ve_kernel::Mesh;

use super::{MAX_VOXELS, Volume, too_big};
use crate::{Fail, fail_suggest};

type V3 = [f64; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn mul(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l > 0.0 { mul(a, 1.0 / l) } else { a }
}

/// Which part of a triangle is nearest to a point.
#[derive(Clone, Copy)]
enum Feature {
    Face,
    Vertex(usize),
    Edge(usize),
}

/// The nearest point on triangle `abc` to `p` and which feature it lies on (Ericson, Real-Time Collision Detection). Edges are 0 ab, 1 bc, 2 ca.
fn closest(p: V3, a: V3, b: V3, c: V3) -> (V3, Feature) {
    let (ab, ac, ap) = (sub(b, a), sub(c, a), sub(p, a));
    let (d1, d2) = (dot(ab, ap), dot(ac, ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return (a, Feature::Vertex(0));
    }
    let bp = sub(p, b);
    let (d3, d4) = (dot(ab, bp), dot(ac, bp));
    if d3 >= 0.0 && d4 <= d3 {
        return (b, Feature::Vertex(1));
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return (add(a, mul(ab, d1 / (d1 - d3))), Feature::Edge(0));
    }
    let cp = sub(p, c);
    let (d5, d6) = (dot(ab, cp), dot(ac, cp));
    if d6 >= 0.0 && d5 <= d6 {
        return (c, Feature::Vertex(2));
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return (add(a, mul(ac, d2 / (d2 - d6))), Feature::Edge(2));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return (add(b, mul(sub(c, b), w)), Feature::Edge(1));
    }
    let denom = 1.0 / (va + vb + vc);
    (
        add(a, add(mul(ab, vb * denom), mul(ac, vc * denom))),
        Feature::Face,
    )
}

pub(super) fn build(
    node: &ve_graph::Node,
    mesh: &Mesh,
    cell: f64,
    band: usize,
) -> Result<Volume, Fail> {
    if mesh.indices.is_empty() {
        return Err(fail_suggest(
            "volume.empty",
            format!("{} has no mesh to sample", node.op),
            "link a closed mesh",
        ));
    }
    // Weld by exact position, so a mesh that repeats vertices along a seam still reads as one surface.
    let mut ids: HashMap<[u32; 3], usize> = HashMap::new();
    let mut points: Vec<V3> = Vec::new();
    let welded: Vec<usize> = mesh
        .positions
        .iter()
        .map(|p| {
            *ids.entry(p.map(f32::to_bits)).or_insert_with(|| {
                points.push(p.map(f64::from));
                points.len() - 1
            })
        })
        .collect();
    let triangles: Vec<[usize; 3]> = mesh
        .indices
        .chunks_exact(3)
        .map(|t| {
            [
                welded[t[0] as usize],
                welded[t[1] as usize],
                welded[t[2] as usize],
            ]
        })
        .filter(|t| t[0] != t[1] && t[1] != t[2] && t[0] != t[2])
        .collect();
    // A closed surface: every edge is shared by exactly two triangles.
    let mut edge_use: HashMap<(usize, usize), u32> = HashMap::new();
    for t in &triangles {
        for e in 0..3 {
            let (a, b) = (t[e], t[(e + 1) % 3]);
            *edge_use.entry((a.min(b), a.max(b))).or_insert(0) += 1;
        }
    }
    let open = edge_use.values().filter(|&&n| n != 2).count();
    if open > 0 {
        return Err(fail_suggest(
            "volume.mesh_open",
            format!(
                "the mesh is not closed: {open} edges are not shared by exactly two faces, so inside and outside are not defined"
            ),
            "close it first (poly.fill_holes, or a solid), then sample it",
        ));
    }
    // Normals for the sign: per face, per edge (sum of its two faces) and per vertex (faces weighted by their angle there).
    let face_normal: Vec<V3> = triangles
        .iter()
        .map(|t| {
            unit(cross(
                sub(points[t[1]], points[t[0]]),
                sub(points[t[2]], points[t[0]]),
            ))
        })
        .collect();
    let mut edge_normal: HashMap<(usize, usize), V3> = HashMap::new();
    let mut vertex_normal: Vec<V3> = vec![[0.0; 3]; points.len()];
    for (t, n) in triangles.iter().zip(&face_normal) {
        for e in 0..3 {
            let (a, b, c) = (t[e], t[(e + 1) % 3], t[(e + 2) % 3]);
            let slot = edge_normal.entry((a.min(b), a.max(b))).or_insert([0.0; 3]);
            *slot = add(*slot, *n);
            let (u, v) = (
                unit(sub(points[b], points[a])),
                unit(sub(points[c], points[a])),
            );
            // The corner's true angle (atan2 of the cross length and the dot): the pseudo-normal is only guaranteed to give the right sign
            // with angle weights. This is the one place that uses a library function beyond `+ - * /` and `sqrt`; a last-bit difference
            // between platforms could only change a sign for a voxel exactly on the edge of a tie.
            let angle = dot(cross(u, v), cross(u, v)).sqrt().atan2(dot(u, v));
            vertex_normal[a] = add(vertex_normal[a], mul(*n, angle));
        }
    }
    // The grid: the mesh's box padded by the band, on the world lattice.
    let trunc = cell * band as f64;
    let (mut low, mut high) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for p in &points {
        for a in 0..3 {
            low[a] = low[a].min(p[a]);
            high[a] = high[a].max(p[a]);
        }
    }
    let pad = trunc + cell;
    let origin = [0, 1, 2].map(|a| ((low[a] - pad) / cell).floor() * cell);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a positive box measured in whole voxels"
    )]
    let dims = [0, 1, 2].map(|a| ((high[a] + pad - origin[a]) / cell).ceil() as usize + 1);
    let total = dims[0].saturating_mul(dims[1]).saturating_mul(dims[2]);
    if total > MAX_VOXELS {
        return Err(too_big(total, cell));
    }
    let to_index = |a: usize, x: f64| -> usize {
        let i = ((x - origin[a]) / cell).floor();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "clamped into the grid"
        )]
        {
            i.clamp(0.0, (dims[a] - 1) as f64) as usize
        }
    };
    // Cost guard: the number of point-triangle tests.
    let mut work: usize = 0;
    let ranges: Vec<[(usize, usize); 3]> = triangles
        .iter()
        .map(|t| {
            let mut r = [(0, 0); 3];
            for a in 0..3 {
                let lo = t
                    .iter()
                    .map(|&i| points[i][a])
                    .fold(f64::INFINITY, f64::min)
                    - trunc;
                let hi = t
                    .iter()
                    .map(|&i| points[i][a])
                    .fold(f64::NEG_INFINITY, f64::max)
                    + trunc;
                r[a] = (to_index(a, lo), to_index(a, hi).min(dims[a] - 1));
            }
            work = work.saturating_add(
                (r[0].1 - r[0].0 + 1) * (r[1].1 - r[1].0 + 1) * (r[2].1 - r[2].0 + 1),
            );
            r
        })
        .collect();
    const MAX_TESTS: usize = 250_000_000;
    if work > MAX_TESTS {
        return Err(fail_suggest(
            "budget.volume",
            format!(
                "sampling would take {work} distance tests (the most is {MAX_TESTS}): too many triangles for this voxel size and band"
            ),
            "raise voxel_size, lower band, or simplify the mesh first (poly.decimate)",
        ));
    }
    let mut values = vec![f32::INFINITY; total];
    let stride_y = dims[0];
    let stride_z = dims[0] * dims[1];
    for (ti, (t, r)) in triangles.iter().zip(&ranges).enumerate() {
        let (a, b, c) = (points[t[0]], points[t[1]], points[t[2]]);
        for k in r[2].0..=r[2].1 {
            for j in r[1].0..=r[1].1 {
                for i in r[0].0..=r[0].1 {
                    let p = [
                        origin[0] + i as f64 * cell,
                        origin[1] + j as f64 * cell,
                        origin[2] + k as f64 * cell,
                    ];
                    let (q, feature) = closest(p, a, b, c);
                    let to_p = sub(p, q);
                    let d = dot(to_p, to_p).sqrt();
                    if d >= trunc {
                        continue;
                    }
                    let at = i + stride_y * j + stride_z * k;
                    if f64::from(values[at].abs()) <= d && values[at].is_finite() {
                        continue;
                    }
                    let n = match feature {
                        Feature::Face => face_normal[ti],
                        Feature::Vertex(v) => vertex_normal[t[v]],
                        Feature::Edge(e) => {
                            edge_normal[&(t[e].min(t[(e + 1) % 3]), t[e].max(t[(e + 1) % 3]))]
                        }
                    };
                    let sign = if dot(to_p, n) < 0.0 { -1.0 } else { 1.0 };
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "distances are stored as f32"
                    )]
                    {
                        values[at] = (sign * d) as f32;
                    }
                }
            }
        }
    }
    // Everything the band did not reach: outside if the border can be reached through it, inside otherwise.
    let mut outside = vec![false; total];
    let mut queue = std::collections::VecDeque::new();
    let corner = 0;
    if values[corner].is_infinite() {
        outside[corner] = true;
        queue.push_back(corner);
    }
    while let Some(at) = queue.pop_front() {
        let (i, j, k) = (at % dims[0], (at / stride_y) % dims[1], at / stride_z);
        let mut visit = |n: usize| {
            if values[n].is_infinite() && !outside[n] {
                outside[n] = true;
                queue.push_back(n);
            }
        };
        if i > 0 {
            visit(at - 1);
        }
        if i + 1 < dims[0] {
            visit(at + 1);
        }
        if j > 0 {
            visit(at - stride_y);
        }
        if j + 1 < dims[1] {
            visit(at + stride_y);
        }
        if k > 0 {
            visit(at - stride_z);
        }
        if k + 1 < dims[2] {
            visit(at + stride_z);
        }
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "distances are stored as f32"
    )]
    let far = trunc as f32;
    for (at, v) in values.iter_mut().enumerate() {
        if v.is_infinite() {
            *v = if outside[at] { far } else { -far };
        }
    }
    Ok(Volume {
        origin,
        cell,
        dims,
        values,
        trunc: far,
    })
}
