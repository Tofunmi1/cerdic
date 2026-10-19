//! UV quality: islands, overlap, coverage, texel density and stretch.

use std::collections::HashMap;

use glam::{Vec2, Vec3};

use crate::error::MeasureError;
use crate::mesh::{Dsu, Mesh};

/// UV coordinates closer than this are treated as the same point when finding islands.
const WELD: f32 = 1e-5;
/// Cells per side of the coverage grid.
const GRID: usize = 128;
/// World or UV area below which a triangle is ignored for density and stretch.
const AREA_FLOOR: f32 = 1e-12;

/// What the UV layout looks like.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UvReport {
    /// Number of connected UV islands.
    pub islands: usize,
    /// The fraction of covered UV area shared by two or more islands (approximate).
    pub overlapping_area_ratio: f32,
    /// The fraction of the unit square that the layout covers, in `0..=1`.
    pub coverage: f32,
    /// The smallest `uv_area / world_area` over the triangles.
    pub texel_density_min: f32,
    /// The median `uv_area / world_area`.
    pub texel_density_median: f32,
    /// The largest `uv_area / world_area`.
    pub texel_density_max: f32,
    /// The mean per-triangle stretch, where 0 is a perfect (conformal) unwrap.
    pub stretch_mean: f32,
    /// The largest per-triangle stretch.
    pub stretch_max: f32,
    /// How many uvs fall outside the unit square.
    pub out_of_range: usize,
}

/// Measure the UV layout of a mesh. The mesh must carry per-vertex uvs.
///
/// Stretch is the ratio of the largest to the smallest singular value of the map from the world
/// triangle to the UV triangle, minus one, so a perfect conformal unwrap reports zero. Overlap and
/// coverage are measured on a [`GRID`]×[`GRID`] raster of the unit square, so they are approximate.
///
/// # Errors
/// Returns [`MeasureError`] when the mesh is malformed or carries no uvs.
pub fn uv_report(mesh: &Mesh) -> Result<UvReport, MeasureError> {
    let triangles = mesh.validate()?;
    let uvs = mesh.uvs.ok_or(MeasureError::MissingUv)?;
    let welded = weld_uvs(uvs);
    let (island, island_count) = island_ids(mesh, triangles, &welded);
    let (covered, overlapped) = rasterize(mesh, uvs, &island);
    let coverage = covered as f32 / (GRID * GRID) as f32;
    let overlapping_area_ratio = if covered > 0 {
        overlapped as f32 / covered as f32
    } else {
        0.0
    };
    let mut densities = Vec::new();
    let mut stretches = Vec::new();
    for t in 0..triangles {
        let (a, b, c) = mesh.corners(t);
        let [ia, ib, ic] = mesh.triangle(t);
        let uva = Vec2::from_array(uvs[ia as usize]);
        let uvb = Vec2::from_array(uvs[ib as usize]);
        let uvc = Vec2::from_array(uvs[ic as usize]);
        let world_area = 0.5 * (b - a).cross(c - a).length();
        let uv_area = 0.5 * ((uvb - uva).perp_dot(uvc - uva)).abs();
        if world_area > AREA_FLOOR {
            densities.push(uv_area / world_area);
        }
        if let Some(stretch) = triangle_stretch(a, b, c, uva, uvb, uvc) {
            stretches.push(stretch);
        }
    }
    densities.sort_by(f32::total_cmp);
    let (texel_density_min, texel_density_median, texel_density_max) = spread(&densities);
    let stretch_mean = if stretches.is_empty() {
        0.0
    } else {
        stretches.iter().sum::<f32>() / stretches.len() as f32
    };
    let stretch_max = stretches.iter().copied().fold(0.0f32, f32::max);
    let out_of_range = uvs.iter().filter(|uv| !in_unit_square(**uv)).count();
    Ok(UvReport {
        islands: island_count,
        overlapping_area_ratio,
        coverage,
        texel_density_min,
        texel_density_median,
        texel_density_max,
        stretch_mean,
        stretch_max,
        out_of_range,
    })
}

fn spread(values: &[f32]) -> (f32, f32, f32) {
    if values.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    (
        values[0],
        values[values.len() / 2],
        values[values.len() - 1],
    )
}

fn in_unit_square(uv: [f32; 2]) -> bool {
    (0.0..=1.0).contains(&uv[0]) && (0.0..=1.0).contains(&uv[1])
}

fn weld_uvs(uvs: &[[f32; 2]]) -> Vec<u32> {
    let mut map: HashMap<(i64, i64), u32> = HashMap::new();
    let mut out = Vec::with_capacity(uvs.len());
    for uv in uvs {
        let key = ((uv[0] / WELD).round() as i64, (uv[1] / WELD).round() as i64);
        let next = map.len() as u32;
        out.push(*map.entry(key).or_insert(next));
    }
    out
}

/// The island id of each triangle, renumbered from zero in triangle order, and the island count.
fn island_ids(mesh: &Mesh, triangles: usize, welded: &[u32]) -> (Vec<u32>, usize) {
    // The set is over triangle ids, not uvs: a mesh with fewer uvs than triangles (per-vertex uvs)
    // would otherwise index past it.
    let mut dsu = Dsu::new(triangles);
    let mut first: HashMap<u32, u32> = HashMap::new();
    let mut ids: Vec<u32> = (0..triangles as u32).collect();
    for t in 0..triangles {
        for vertex in mesh.triangle(t) {
            let w = welded[vertex as usize];
            match first.get(&w) {
                Some(&other) => dsu.union(t as u32, other),
                None => {
                    first.insert(w, t as u32);
                }
            }
        }
    }
    let mut renumbered: HashMap<u32, u32> = HashMap::new();
    let mut next = 0u32;
    for id in &mut ids {
        let root = dsu.find(*id);
        *id = *renumbered.entry(root).or_insert_with(|| {
            let assigned = next;
            next += 1;
            assigned
        });
    }
    (ids, next as usize)
}

fn rasterize(mesh: &Mesh, uvs: &[[f32; 2]], island: &[u32]) -> (usize, usize) {
    let mut owner = vec![-1i32; GRID * GRID];
    let mut covered = 0;
    let mut overlapped = 0;
    for (t, &id) in island.iter().enumerate() {
        let [ia, ib, ic] = mesh.triangle(t);
        let a = uvs[ia as usize];
        let b = uvs[ib as usize];
        let c = uvs[ic as usize];
        let id = id as i32;
        let (x0, x1) = cell_range(a[0].min(b[0]).min(c[0]), a[0].max(b[0]).max(c[0]));
        let (y0, y1) = cell_range(a[1].min(b[1]).min(c[1]), a[1].max(b[1]).max(c[1]));
        for gy in y0..=y1 {
            for gx in x0..=x1 {
                let centre = [
                    (gx as f32 + 0.5) / GRID as f32,
                    (gy as f32 + 0.5) / GRID as f32,
                ];
                if !covers(centre, a, b, c) {
                    continue;
                }
                let cell = gy * GRID + gx;
                if owner[cell] == -1 {
                    owner[cell] = id;
                    covered += 1;
                } else if owner[cell] != id && owner[cell] != -2 {
                    owner[cell] = -2;
                    overlapped += 1;
                }
            }
        }
    }
    (covered, overlapped)
}

fn cell_range(lo: f32, hi: f32) -> (usize, usize) {
    let to_cell = |v: f32| (v * GRID as f32).clamp(0.0, (GRID - 1) as f32) as usize;
    if hi < 0.0 || lo > 1.0 {
        // Fully outside the unit square: an empty range.
        return (1, 0);
    }
    (to_cell(lo), to_cell(hi))
}

fn covers(p: [f32; 2], a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> bool {
    let d1 = sign2(p, a, b);
    let d2 = sign2(p, b, c);
    let d3 = sign2(p, c, a);
    let negative = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let positive = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(negative && positive)
}

fn sign2(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    (p[0] - b[0]) * (a[1] - b[1]) - (a[0] - b[0]) * (p[1] - b[1])
}

/// The per-triangle stretch: `sigma_max / sigma_min - 1`, or `None` for a degenerate triangle.
fn triangle_stretch(a: Vec3, b: Vec3, c: Vec3, uva: Vec2, uvb: Vec2, uvc: Vec2) -> Option<f32> {
    let e1 = b - a;
    let e2 = c - a;
    let normal = e1.cross(e2);
    if normal.length_squared() <= AREA_FLOOR {
        return None;
    }
    let tangent = e1.normalize();
    let bitangent = normal.cross(tangent).normalize();
    let (e11, e12) = (e1.dot(tangent), e1.dot(bitangent));
    let (e21, e22) = (e2.dot(tangent), e2.dot(bitangent));
    let det = e11 * e22 - e21 * e12;
    if det.abs() <= AREA_FLOOR {
        return None;
    }
    let (f11, f21) = (uvb.x - uva.x, uvb.y - uva.y);
    let (f12, f22) = (uvc.x - uva.x, uvc.y - uva.y);
    let inv = [e22 / det, -e21 / det, -e12 / det, e11 / det];
    let j11 = f11 * inv[0] + f12 * inv[2];
    let j12 = f11 * inv[1] + f12 * inv[3];
    let j21 = f21 * inv[0] + f22 * inv[2];
    let j22 = f21 * inv[1] + f22 * inv[3];
    let g11 = j11 * j11 + j21 * j21;
    let g12 = j11 * j12 + j21 * j22;
    let g22 = j12 * j12 + j22 * j22;
    let trace = g11 + g22;
    let disc = ((g11 - g22) * (g11 - g22) + 4.0 * g12 * g12).sqrt();
    let lambda_min = 0.5 * (trace - disc);
    if lambda_min <= 0.0 {
        return None;
    }
    let ratio = ((0.5 * (trace + disc)) / lambda_min).sqrt();
    Some(ratio - 1.0)
}
