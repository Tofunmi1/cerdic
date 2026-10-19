//! How good a UV layout is: stretch, flipped faces, overlap and how much of the unit square it uses.

use crate::{AttrData, PolyMesh};

/// Side of the grid used to look for overlapping UV triangles.
const GRID: usize = 128;

/// Measurements of the corner attributes `u` and `v`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UvQuality {
    /// Faces that have a measurable area both on the surface and in UV space.
    pub measured_faces: usize,
    /// Texel density of the most stretched face over the mean density (1 is even, 4 is four times too dense).
    pub max_density_ratio: f64,
    /// Texel density of the most squashed face over the mean density (1 is even, 0.25 is a quarter).
    pub min_density_ratio: f64,
    /// Faces drawn mirrored relative to most faces.
    pub flipped_faces: usize,
    /// Share of covered texels that two or more faces cover (0 when islands do not overlap).
    pub overlap: f64,
    /// Share of the unit square the layout covers.
    pub coverage: f64,
}

fn corner_attribute<'a>(mesh: &'a PolyMesh, name: &str) -> Option<&'a [f32]> {
    match &mesh.attributes().get(name)?.data {
        AttrData::F32(values) => Some(values),
        _ => None,
    }
}

fn triangle_area_3d(p: [[f64; 3]; 3]) -> f64 {
    let a = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
    let b = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
    let c = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    0.5 * c.iter().map(|v| v * v).sum::<f64>().sqrt()
}

fn signed_area_2d(t: [[f64; 2]; 3]) -> f64 {
    0.5 * ((t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[2][0] - t[0][0]) * (t[1][1] - t[0][1]))
}

/// Measures the layout, or `None` when the mesh has no `u` and `v` corner attributes.
#[must_use]
#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "grid cells (128 per side) and face counts are far below the float limits"
)]
pub fn uv_quality(mesh: &PolyMesh) -> Option<UvQuality> {
    let u = corner_attribute(mesh, "u")?;
    let v = corner_attribute(mesh, "v")?;
    let positions = mesh.positions();
    // Every face is fanned into triangles; each triangle keeps its surface and UV area.
    let mut triangles: Vec<([[f64; 2]; 3], f64, f64)> = Vec::new();
    let mut at = 0_usize;
    for &size in mesh.face_sizes() {
        let corners = &mesh.corners()[at..at + size as usize];
        for k in 1..size as usize - 1 {
            let ids = [0, k, k + 1];
            let uv = ids.map(|i| [f64::from(u[at + i]), f64::from(v[at + i])]);
            let p = ids.map(|i| positions[corners[i] as usize].map(f64::from));
            triangles.push((uv, triangle_area_3d(p), signed_area_2d(uv)));
        }
        at += size as usize;
    }
    let usable: Vec<_> = triangles
        .iter()
        .filter(|t| t.1 > 1e-12 && t.2.abs() > 1e-12)
        .collect();
    if usable.is_empty() {
        return None;
    }
    let (surface, uv_area): (f64, f64) = usable
        .iter()
        .fold((0.0, 0.0), |s, t| (s.0 + t.1, s.1 + t.2.abs()));
    let mean = uv_area / surface;
    let ratios: Vec<f64> = usable.iter().map(|t| t.2.abs() / t.1 / mean).collect();
    let positive = usable.iter().filter(|t| t.2 > 0.0).count();
    let flipped = positive.min(usable.len() - positive);
    // Overlap and coverage on a grid of texel centres.
    let mut count = vec![0_u8; GRID * GRID];
    for (uv, _, area) in &usable.iter().map(|t| (t.0, t.1, t.2)).collect::<Vec<_>>() {
        let (lo, hi) = (
            [0, 1].map(|a| uv.iter().map(|p| p[a]).fold(f64::INFINITY, f64::min)),
            [0, 1].map(|a| uv.iter().map(|p| p[a]).fold(f64::NEG_INFINITY, f64::max)),
        );
        let cell = |x: f64| ((x * GRID as f64).floor().max(0.0) as usize).min(GRID - 1);
        for gy in cell(lo[1])..=cell(hi[1]) {
            for gx in cell(lo[0])..=cell(hi[0]) {
                let p = [
                    (gx as f64 + 0.5) / GRID as f64,
                    (gy as f64 + 0.5) / GRID as f64,
                ];
                let w0 = signed_area_2d([p, uv[1], uv[2]]) / area;
                let w1 = signed_area_2d([uv[0], p, uv[2]]) / area;
                if w0 >= 0.0 && w1 >= 0.0 && w0 + w1 <= 1.0 {
                    let slot = &mut count[gy * GRID + gx];
                    *slot = slot.saturating_add(1);
                }
            }
        }
    }
    let covered = count.iter().filter(|&&c| c > 0).count();
    let overlapped = count.iter().filter(|&&c| c > 1).count();
    Some(UvQuality {
        measured_faces: usable.len(),
        max_density_ratio: ratios.iter().copied().fold(0.0, f64::max),
        min_density_ratio: ratios.iter().copied().fold(f64::INFINITY, f64::min),
        flipped_faces: flipped,
        overlap: if covered == 0 {
            0.0
        } else {
            overlapped as f64 / covered as f64
        },
        coverage: covered as f64 / (GRID * GRID) as f64,
    })
}
