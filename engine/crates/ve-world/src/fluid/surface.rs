//! The surface of a liquid: particles are splatted into a density grid (each adds a smooth bump) and the level where the density falls to a share of
//! a settled liquid's is meshed by surface nets (one vertex per grid cell the surface crosses, one quad per crossed grid edge, split into two
//! triangles). Normals come from the density's gradient, so the surface is smooth without a smoothing pass. Arithmetic only, in a fixed order.

use crate::cloth::V3;

/// Most grid points one surface may use.
pub const MAX_GRID: usize = 2_000_000;

/// A triangle mesh with a normal per vertex, facing out of the liquid.
#[derive(Clone, Debug, Default)]
pub struct Surface {
    /// Vertex positions.
    pub positions: Vec<V3>,
    /// Vertex normals (unit length).
    pub normals: Vec<V3>,
    /// Triangles as vertex numbers, wound counter-clockwise seen from outside.
    pub triangles: Vec<[u32; 3]>,
}

/// The twelve edges of a cell as pairs of corner numbers; corner bits are x 1, y 2, z 4.
const EDGES: [(usize, usize); 12] = [
    (0, 1),
    (2, 3),
    (4, 5),
    (6, 7),
    (0, 2),
    (1, 3),
    (4, 6),
    (5, 7),
    (0, 4),
    (1, 5),
    (2, 6),
    (3, 7),
];

fn bump(q2: f32) -> f32 {
    let d = 1.0 - q2;
    if d <= 0.0 { 0.0 } else { d * d * d }
}

/// The surface of the liquid made of `points` that rest `spacing` apart, meshed on a grid of `cell` metres. `level` is the share of a settled
/// liquid's density at which the surface sits (0.4 puts it a little outside the outer particles' centres). `None` when there are no points, the
/// grid would exceed `MAX_GRID` points, or there is no surface.
#[must_use]
pub fn surface(points: &[V3], spacing: f32, cell: f32, level: f32) -> Option<Surface> {
    if points.is_empty()
        || !(spacing > 0.0 && cell > 0.0)
        || !points.iter().flatten().all(|v| v.is_finite())
    {
        return None;
    }
    let reach = 1.8 * spacing;
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in points {
        for a in 0..3 {
            lo[a] = lo[a].min(p[a]);
            hi[a] = hi[a].max(p[a]);
        }
    }
    let origin = [0, 1, 2].map(|a| lo[a] - reach - cell);
    let dims: Vec<usize> = (0..3)
        .map(|a| ((hi[a] + reach + cell - origin[a]) / cell).ceil() as usize + 1)
        .collect();
    let total = dims[0].checked_mul(dims[1])?.checked_mul(dims[2])?;
    if total > MAX_GRID || dims.iter().any(|&d| d < 2) {
        return None;
    }
    let at = |i: usize, j: usize, k: usize| i + dims[0] * (j + dims[1] * k);
    let mut density = vec![0.0_f32; total];
    let span = (reach / cell).ceil() as isize;
    for p in points {
        let c = [0, 1, 2].map(|a| ((p[a] - origin[a]) / cell).round() as isize);
        for k in (c[2] - span).max(0)..=(c[2] + span).min(dims[2] as isize - 1) {
            for j in (c[1] - span).max(0)..=(c[1] + span).min(dims[1] as isize - 1) {
                for i in (c[0] - span).max(0)..=(c[0] + span).min(dims[0] as isize - 1) {
                    let d = [
                        origin[0] + i as f32 * cell - p[0],
                        origin[1] + j as f32 * cell - p[1],
                        origin[2] + k as f32 * cell - p[2],
                    ];
                    density[at(i as usize, j as usize, k as usize)] +=
                        bump((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) / (reach * reach));
                }
            }
        }
    }
    // The density of a settled liquid, summed over a lattice neighbourhood, sets the level.
    let mut settled = 0.0;
    for k in -2_i32..=2 {
        for j in -2_i32..=2 {
            for i in -2_i32..=2 {
                let d = [i as f32 * spacing, j as f32 * spacing, k as f32 * spacing];
                settled += bump((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) / (reach * reach));
            }
        }
    }
    let iso = level * settled;
    // Negative inside, like a signed distance.
    let value = |i: usize, j: usize, k: usize| iso - density[at(i, j, k)];
    let cells = [dims[0] - 1, dims[1] - 1, dims[2] - 1];
    let cell_id = |i: usize, j: usize, k: usize| i + cells[0] * (j + cells[1] * k);
    let mut cell_vertex = vec![u32::MAX; cells[0] * cells[1] * cells[2]];
    let mut positions: Vec<V3> = Vec::new();
    for k in 0..cells[2] {
        for j in 0..cells[1] {
            for i in 0..cells[0] {
                let mut corner = [0.0_f32; 8];
                let mut inside = 0;
                for (c, slot) in corner.iter_mut().enumerate() {
                    *slot = value(i + (c & 1), j + ((c >> 1) & 1), k + ((c >> 2) & 1));
                    inside += usize::from(*slot < 0.0);
                }
                if inside == 0 || inside == 8 {
                    continue;
                }
                let (mut sum, mut crossings) = ([0.0_f32; 3], 0.0_f32);
                for &(a, b) in &EDGES {
                    if (corner[a] < 0.0) == (corner[b] < 0.0) {
                        continue;
                    }
                    let t = corner[a] / (corner[a] - corner[b]);
                    for axis in 0..3 {
                        let (ca, cb) = (((a >> axis) & 1) as f32, ((b >> axis) & 1) as f32);
                        sum[axis] += ca + (cb - ca) * t;
                    }
                    crossings += 1.0;
                }
                let base = [i, j, k];
                cell_vertex[cell_id(i, j, k)] = positions.len() as u32;
                positions.push(
                    [0, 1, 2].map(|a| origin[a] + (base[a] as f32 + sum[a] / crossings) * cell),
                );
            }
        }
    }
    let mut triangles: Vec<[u32; 3]> = Vec::new();
    let mut push = |ring: [Option<(usize, usize, usize)>; 4], flip: bool| {
        let ids: Vec<u32> = ring
            .iter()
            .filter_map(|c| c.map(|(i, j, k)| cell_vertex[cell_id(i, j, k)]))
            .collect();
        if ids.len() != 4 || ids.contains(&u32::MAX) {
            return;
        }
        let q = if flip {
            [ids[3], ids[2], ids[1], ids[0]]
        } else {
            [ids[0], ids[1], ids[2], ids[3]]
        };
        triangles.push([q[0], q[1], q[2]]);
        triangles.push([q[0], q[2], q[3]]);
    };
    let ok = |i: isize, j: isize, k: isize| {
        (i >= 0
            && j >= 0
            && k >= 0
            && (i as usize) < cells[0]
            && (j as usize) < cells[1]
            && (k as usize) < cells[2])
            .then_some((i as usize, j as usize, k as usize))
    };
    for k in 0..dims[2] {
        for j in 0..dims[1] {
            for i in 0..dims[0] {
                let here = value(i, j, k);
                let (si, sj, sk) = (i as isize, j as isize, k as isize);
                if i + 1 < dims[0] && (here < 0.0) != (value(i + 1, j, k) < 0.0) {
                    push(
                        [
                            ok(si, sj - 1, sk - 1),
                            ok(si, sj, sk - 1),
                            ok(si, sj, sk),
                            ok(si, sj - 1, sk),
                        ],
                        here >= 0.0,
                    );
                }
                if j + 1 < dims[1] && (here < 0.0) != (value(i, j + 1, k) < 0.0) {
                    push(
                        [
                            ok(si - 1, sj, sk - 1),
                            ok(si - 1, sj, sk),
                            ok(si, sj, sk),
                            ok(si, sj, sk - 1),
                        ],
                        here >= 0.0,
                    );
                }
                if k + 1 < dims[2] && (here < 0.0) != (value(i, j, k + 1) < 0.0) {
                    push(
                        [
                            ok(si - 1, sj - 1, sk),
                            ok(si, sj - 1, sk),
                            ok(si, sj, sk),
                            ok(si - 1, sj, sk),
                        ],
                        here >= 0.0,
                    );
                }
            }
        }
    }
    if triangles.is_empty() {
        return None;
    }
    // Normals: the way the density falls, sampled at the vertex by central differences on the grid.
    let sample = |p: V3| {
        let c = [0, 1, 2].map(|a| {
            (((p[a] - origin[a]) / cell).round() as isize).clamp(1, dims[a] as isize - 2) as usize
        });
        let g = |i: usize, j: usize, k: usize| density[at(i, j, k)];
        [
            g(c[0] - 1, c[1], c[2]) - g(c[0] + 1, c[1], c[2]),
            g(c[0], c[1] - 1, c[2]) - g(c[0], c[1] + 1, c[2]),
            g(c[0], c[1], c[2] - 1) - g(c[0], c[1], c[2] + 1),
        ]
    };
    let normals = positions
        .iter()
        .map(|p| {
            let n = sample(*p);
            let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if length > 1.0e-9 {
                [n[0] / length, n[1] / length, n[2] / length]
            } else {
                [0.0, 1.0, 0.0]
            }
        })
        .collect();
    Some(Surface {
        positions,
        normals,
        triangles,
    })
}
