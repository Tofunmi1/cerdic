//! Minimum wall thickness, measured by inward rays from seeded sample points.

use glam::Vec3;

use crate::MAX_TRIANGLES;
use crate::bvh::Bvh;
use crate::error::MeasureError;
use crate::mesh::Mesh;

/// How many of the thinnest samples are returned as the worst locations.
const WORST: usize = 8;
/// The fixed seed for area-weighted sampling, so results repeat exactly.
const SEED: u64 = 0x5EED_0000_0000_0001;
/// How far from the surface the ray starts, as a fraction of the mesh diagonal.
const OFFSET_FRACTION: f32 = 1e-4;

/// One sample and the wall thickness measured there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SampleHit {
    /// Where the ray started, in metres.
    pub position: [f32; 3],
    /// The thickness it measured, in metres.
    pub thickness: f32,
}

/// Wall-thickness statistics.
#[derive(Clone, Debug, PartialEq)]
pub struct Thickness {
    /// The thinnest wall found, in metres.
    pub min: f32,
    /// The 5th percentile thickness, in metres.
    pub percentile_5: f32,
    /// The thinnest few samples, ascending, so the caller can find the thin spot.
    pub worst: Vec<SampleHit>,
    /// How many of the requested samples hit a far wall.
    pub samples_hit: usize,
    /// How many samples were requested.
    pub samples: usize,
}

/// Measure wall thickness by casting inward rays from seeded, area-weighted sample points.
///
/// Refuses meshes above [`MAX_TRIANGLES`]. The surface orientation (positive signed volume means
/// outward faces) tells the tool which way is "in", so a flipped mesh still measures its walls.
///
/// # Errors
/// Returns [`MeasureError`] when the mesh is malformed, above the triangle budget, has no area, has
/// no interior wall to hit, or `samples` is zero.
pub fn min_wall_thickness(mesh: &Mesh, samples: usize) -> Result<Thickness, MeasureError> {
    let triangles = mesh.validate()?;
    if triangles > MAX_TRIANGLES {
        return Err(MeasureError::TriangleBudget {
            triangles,
            limit: MAX_TRIANGLES,
        });
    }
    if samples == 0 {
        return Err(MeasureError::Degenerate("no thickness samples requested"));
    }
    let bvh = Bvh::build(mesh)?;
    let orientation = if mesh.signed_volume() >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let offset = (mesh.diagonal() * OFFSET_FRACTION).max(1e-6);
    let cumulative = cumulative_areas(mesh, triangles);
    let total = cumulative.last().copied().unwrap_or(0.0);
    if total <= 0.0 {
        return Err(MeasureError::Degenerate("the mesh has no area"));
    }
    let mut rng = Rng::new(SEED);
    let mut hits: Vec<SampleHit> = Vec::new();
    for _ in 0..samples {
        let target = rng.next_f32() * total;
        let t = pick_triangle(&cumulative, target);
        let (a, b, c) = mesh.corners(t);
        let point = sample_point(a, b, c, &mut rng);
        let inward = -((b - a).cross(c - a)).normalize_or_zero() * orientation;
        let origin = point + inward * offset;
        if let Some(hit) = bvh.raycast(mesh, origin.to_array(), inward.to_array()) {
            hits.push(SampleHit {
                position: point.to_array(),
                thickness: hit.distance + offset,
            });
        }
    }
    if hits.is_empty() {
        return Err(MeasureError::NoThickness);
    }
    hits.sort_by(|x, y| x.thickness.total_cmp(&y.thickness));
    let index = ((hits.len() * 5) / 100).min(hits.len() - 1);
    let min = hits[0].thickness;
    let percentile_5 = hits[index].thickness;
    let worst = hits.iter().take(WORST).copied().collect();
    Ok(Thickness {
        min,
        percentile_5,
        worst,
        samples_hit: hits.len(),
        samples,
    })
}

fn pick_triangle(cumulative: &[f32], target: f32) -> usize {
    cumulative
        .partition_point(|&c| c < target)
        .min(cumulative.len() - 1)
}

fn cumulative_areas(mesh: &Mesh, triangles: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(triangles);
    let mut sum = 0.0;
    for t in 0..triangles {
        let (a, b, c) = mesh.corners(t);
        sum += 0.5 * (b - a).cross(c - a).length();
        out.push(sum);
    }
    out
}

fn sample_point(a: Vec3, b: Vec3, c: Vec3, rng: &mut Rng) -> Vec3 {
    let r1 = rng.next_f32();
    let r2 = rng.next_f32();
    let sqrt = r1.sqrt();
    a * (1.0 - sqrt) + b * (sqrt * (1.0 - r2)) + c * (sqrt * r2)
}

/// `SplitMix64`, a tiny deterministic generator.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / 16_777_216.0
    }
}
