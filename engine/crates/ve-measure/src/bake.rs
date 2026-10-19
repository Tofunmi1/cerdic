//! Texture baking: a tangent-space normal map and an ambient-occlusion map of a low-poly mesh, measured against a
//! high-poly one. Pure functions on plain arrays, deterministic, bounded.
//!
//! The low mesh's UV layout defines the image: row `y` is `v = (y + 0.5) / size`, column `x` is `u = (x + 0.5) / size`.
//! Texels covered by a UV triangle are shaded; the nearest covered texels (within [`PADDING`]) are copied outward so
//! filtering at island borders does not bleed in the background, and everything else is transparent.

mod frame;

use glam::Vec3;

use crate::bvh::Bvh;
use crate::error::MeasureError;
use crate::mesh::Mesh;
use frame::{Frame, hemisphere, smooth_normals, texel_turn, uv_directions};

/// Smallest image side a bake accepts.
pub const MIN_SIZE: u32 = 8;
/// Largest image side a bake accepts.
pub const MAX_SIZE: u32 = 2048;
/// Most rays per texel for ambient occlusion.
pub const MAX_RAYS: u32 = 256;
/// How far, in texels, the covered area is copied outward.
pub const PADDING: i64 = 4;
/// Most rays a whole bake may cast (`size * size * rays`): 1024 x 1024 at 256 rays fits, 2048 x 2048 at 256 does not.
const MAX_WORK: u64 = 500_000_000;

/// The low-poly mesh whose UV layout is baked: positions, triangle indices and one UV per triangle corner.
#[derive(Clone, Copy, Debug)]
pub struct LowMesh<'a> {
    /// Vertex positions, in metres.
    pub positions: &'a [[f32; 3]],
    /// Triangle indices; the length is a multiple of three.
    pub indices: &'a [u32],
    /// One `[u, v]` per index (per triangle corner), in the order of `indices`.
    pub corner_uvs: &'a [[f32; 2]],
}

impl<'a> LowMesh<'a> {
    /// Checks the mesh: well formed, finite, and a UV for every corner.
    ///
    /// # Errors
    /// [`MeasureError`] for a malformed mesh, or [`MeasureError::Bake`] when the UVs do not match the corners.
    pub fn new(
        positions: &'a [[f32; 3]],
        indices: &'a [u32],
        corner_uvs: &'a [[f32; 2]],
    ) -> Result<Self, MeasureError> {
        Mesh::new(positions, indices).triangle_count()?;
        if corner_uvs.len() != indices.len() {
            return Err(MeasureError::Bake(
                "there must be one uv per triangle corner",
            ));
        }
        if corner_uvs
            .iter()
            .any(|uv| !uv.iter().all(|c| c.is_finite()))
        {
            return Err(MeasureError::Bake("a uv is not finite"));
        }
        Ok(Self {
            positions,
            indices,
            corner_uvs,
        })
    }
}

/// A baked image: `size * size` RGBA pixels, row 0 at v = 0.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bake {
    /// Side of the square image, in pixels.
    pub size: u32,
    /// RGBA, four bytes a pixel; alpha is 255 on covered and padded pixels and 0 elsewhere.
    pub rgba: Vec<u8>,
    /// How many pixels a UV triangle covers (padding not counted).
    pub covered: u32,
}

/// The surface at one covered texel.
struct Texel {
    x: u32,
    y: u32,
    point: Vec3,
    frame: Frame,
}

fn encode(v: f32) -> u8 {
    ((v * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8
}

fn check_size(size: u32) -> Result<(), MeasureError> {
    if (MIN_SIZE..=MAX_SIZE).contains(&size) {
        Ok(())
    } else {
        Err(MeasureError::Bake(
            "the image side must be 8 to 2048 pixels",
        ))
    }
}

/// Bakes the high mesh's normals into the low mesh's tangent space (OpenGL convention, green up).
///
/// For each texel a ray starts `cage` above the low surface and travels against its normal; the nearest hit on the high
/// mesh within `cage + max_distance` supplies the normal, written as `(n * 0.5 + 0.5) * 255`. A miss writes the flat
/// normal (128, 128, 255).
///
/// # Errors
/// [`MeasureError`] for a malformed mesh, a size outside 8 to 2048, or a non-positive `cage` or `max_distance`.
pub fn bake_normals(
    low: &LowMesh,
    high: &Mesh,
    size: u32,
    cage: f32,
    max_distance: f32,
) -> Result<Bake, MeasureError> {
    check_size(size)?;
    if !(cage.is_finite() && cage >= 0.0 && max_distance.is_finite() && max_distance > 0.0) {
        return Err(MeasureError::Bake(
            "cage must be zero or more and max_distance above zero",
        ));
    }
    high.triangle_count()?;
    let bvh = Bvh::build(high)?;
    Ok(walk(low, size, |texel| {
        let down = -texel.frame.normal;
        let hit = bvh
            .raycast(
                high,
                (texel.point - down * cage).to_array(),
                down.to_array(),
            )
            .filter(|hit| hit.distance <= cage + max_distance);
        let local = hit.map_or(Vec3::Z, |hit| {
            let mut normal = Vec3::from_array(hit.normal).normalize_or_zero();
            // Face the ray's origin whatever the high mesh's winding.
            if normal.dot(down) > 0.0 {
                normal = -normal;
            }
            texel.frame.to_local(normal)
        });
        [encode(local.x), encode(local.y), encode(local.z)]
    }))
}

/// Bakes ambient occlusion of the high mesh onto the low mesh's UV layout, as grey.
///
/// From each texel's surface point (lifted `bias` along the normal) `rays` cosine-weighted directions are cast against
/// the high mesh; the value is the fraction that travel `max_distance` without a hit (255 is fully open).
///
/// # Errors
/// [`MeasureError`] for a malformed mesh, a size outside 8 to 2048, `rays` outside 1 to 256, a bake that would cast
/// more than half a billion rays, or a negative `bias` or non-positive `max_distance`.
pub fn bake_ao(
    low: &LowMesh,
    high: &Mesh,
    size: u32,
    rays: u32,
    max_distance: f32,
    bias: f32,
) -> Result<Bake, MeasureError> {
    check_size(size)?;
    if !(1..=MAX_RAYS).contains(&rays) {
        return Err(MeasureError::Bake("rays must be 1 to 256"));
    }
    if u64::from(size) * u64::from(size) * u64::from(rays) > MAX_WORK {
        return Err(MeasureError::Bake(
            "size * size * rays is above half a billion rays: lower the size or the rays",
        ));
    }
    if !(bias.is_finite() && bias >= 0.0 && max_distance.is_finite() && max_distance > 0.0) {
        return Err(MeasureError::Bake(
            "bias must be zero or more and max_distance above zero",
        ));
    }
    high.triangle_count()?;
    let bvh = Bvh::build(high)?;
    Ok(walk(low, size, |texel| {
        let origin = texel.point + texel.frame.normal * bias;
        let turn = texel_turn(texel.x, texel.y);
        let open = (0..rays)
            .filter(|&i| {
                let direction = texel.frame.to_world(hemisphere(i, rays, turn));
                bvh.raycast(high, origin.to_array(), direction.to_array())
                    .is_none_or(|hit| hit.distance > max_distance)
            })
            .count();
        let grey = (open as f32 / rays as f32 * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8;
        [grey; 3]
    }))
}

/// Visits every texel a low-mesh UV triangle covers (the first triangle in order wins an overlap), writes what `shade`
/// returns, then pads outward.
fn walk(low: &LowMesh, pixels: u32, mut shade: impl FnMut(&Texel) -> [u8; 3]) -> Bake {
    let normals = smooth_normals(low.positions, low.indices);
    let side = pixels as usize;
    let mut rgba = vec![0_u8; side * side * 4];
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.copy_from_slice(&[128, 128, 255, 0]);
    }
    let mut covered = vec![false; side * side];
    let scale = pixels as f32;
    for (t, tri) in low.indices.chunks_exact(3).enumerate() {
        let corner = [tri[0], tri[1], tri[2]];
        let p = corner.map(|i| Vec3::from_array(low.positions[i as usize]));
        let n = corner.map(|i| normals[i as usize]);
        let uv = [0, 1, 2].map(|k| low.corner_uvs[t * 3 + k]);
        let Some((tangent, bitangent)) = uv_directions(p, uv) else {
            continue;
        };
        let face = (p[1] - p[0]).cross(p[2] - p[0]);
        let det = (uv[1][0] - uv[0][0]) * (uv[2][1] - uv[0][1])
            - (uv[2][0] - uv[0][0]) * (uv[1][1] - uv[0][1]);
        let range = |axis: usize| {
            let lo = uv.iter().map(|c| c[axis]).fold(f32::INFINITY, f32::min);
            let hi = uv.iter().map(|c| c[axis]).fold(f32::NEG_INFINITY, f32::max);
            let last = side.saturating_sub(1) as f32;
            ((lo * scale - 0.5).floor().clamp(0.0, last) as usize)
                ..=((hi * scale + 0.5).ceil().clamp(0.0, last) as usize)
        };
        for y in range(1) {
            for x in range(0) {
                if covered[y * side + x] {
                    continue;
                }
                let at = [(x as f32 + 0.5) / scale, (y as f32 + 0.5) / scale];
                let w0 = ((uv[1][0] - at[0]) * (uv[2][1] - at[1])
                    - (uv[2][0] - at[0]) * (uv[1][1] - at[1]))
                    / det;
                let w1 = ((uv[2][0] - at[0]) * (uv[0][1] - at[1])
                    - (uv[0][0] - at[0]) * (uv[2][1] - at[1]))
                    / det;
                let w2 = 1.0 - w0 - w1;
                if w0 < -1.0e-6 || w1 < -1.0e-6 || w2 < -1.0e-6 {
                    continue;
                }
                let mut normal = n[0] * w0 + n[1] * w1 + n[2] * w2;
                if normal.length_squared() < 1.0e-12 {
                    normal = face;
                }
                let texel = Texel {
                    x: x as u32,
                    y: y as u32,
                    point: p[0] * w0 + p[1] * w1 + p[2] * w2,
                    frame: Frame::new(normal, tangent, bitangent),
                };
                let [r, g, b] = shade(&texel);
                rgba[(y * side + x) * 4..][..4].copy_from_slice(&[r, g, b, 255]);
                covered[y * side + x] = true;
            }
        }
    }
    let count = covered.iter().filter(|c| **c).count();
    pad(&mut rgba, &covered, side);
    Bake {
        size: pixels,
        rgba,
        covered: u32::try_from(count).unwrap_or(u32::MAX),
    }
}

/// Copies each uncovered texel's nearest covered neighbour within [`PADDING`] (looking at the covered set as it was
/// before padding, so the result does not depend on the scan order).
fn pad(rgba: &mut [u8], covered: &[bool], side: usize) {
    let snapshot = rgba.to_vec();
    let reach = PADDING;
    for y in 0..side as i64 {
        for x in 0..side as i64 {
            if covered[y as usize * side + x as usize] {
                continue;
            }
            let mut best: Option<(i64, usize)> = None;
            for dy in -reach..=reach {
                for dx in -reach..=reach {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= side as i64 || ny >= side as i64 {
                        continue;
                    }
                    let source = ny as usize * side + nx as usize;
                    let distance = dx * dx + dy * dy;
                    if covered[source] && best.is_none_or(|(d, _)| distance < d) {
                        best = Some((distance, source));
                    }
                }
            }
            if let Some((_, source)) = best {
                let target = (y as usize * side + x as usize) * 4;
                rgba[target..target + 3].copy_from_slice(&snapshot[source * 4..source * 4 + 3]);
                rgba[target + 3] = 255;
            }
        }
    }
}
