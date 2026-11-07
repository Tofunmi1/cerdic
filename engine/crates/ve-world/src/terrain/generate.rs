//! World-space generators and shaping operations.
use super::{HeightGrid, TerrainError};
use crate::noise::{Basis, Fractal, FractalKind, Noise as SharedNoise, Warp, WarpKind};
use crate::num::{cell_f32, count_f32, floor_index, lattice_f32, lattice_index, offset};

/// Fractal noise settings. Heights are evaluated at WORLD coordinates so tiles match.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Noise {
    /// Seed.
    pub seed: u32,
    /// Feature size in metres of the lowest octave.
    pub scale: f32,
    /// Octaves 1..=8.
    pub octaves: u32,
    /// Amplitude multiplier per octave, 0..=1.
    pub roughness: f32,
    /// Frequency multiplier per octave, 1.5..=3.
    pub lacunarity: f32,
    /// Ridged multifractal instead of plain fractal.
    pub ridged: bool,
    /// Domain-warp strength in metres, 0 for none.
    pub warp: f32,
    /// `None` keeps the original gradient noise (so existing worlds do not change); `Some` uses [`crate::noise`] with this basis.
    pub basis: Option<Basis>,
}

/// The shared noise equivalent of the terrain settings.
fn shared_noise(noise: &Noise, basis: Basis) -> SharedNoise {
    SharedNoise {
        basis,
        seed: noise.seed,
        frequency: 1.0 / f64::from(noise.scale),
        fractal: Some(Fractal {
            kind: if noise.ridged {
                FractalKind::Ridged
            } else {
                FractalKind::Fbm
            },
            octaves: u8::try_from(noise.octaves).unwrap_or(8),
            lacunarity: f64::from(noise.lacunarity),
            gain: f64::from(noise.roughness),
            weighted_strength: 0.0,
        }),
        warp: (noise.warp > 0.0).then(|| Warp {
            kind: WarpKind::Gradient,
            amplitude: f64::from(noise.warp / noise.scale),
            frequency: 0.5,
            octaves: 1,
        }),
    }
}

/// A shared-noise sample in `[-1, 1]` as a height fraction in `[0, 1]`.
fn fraction(sample: f64) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the sample is in [-1, 1]; f32 holds it"
    )]
    let narrow = ((sample + 1.0) * 0.5) as f32;
    narrow
}

/// The profile of a [`stamp`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StampShape {
    /// Smooth hill (smoothstep falloff).
    Smooth,
    /// Straight-sided cone.
    Cone,
    /// Flat top with smooth sides.
    Plateau,
    /// Bowl with a raised rim.
    Crater,
}

fn hash(seed: u32, x: i32, z: i32) -> u32 {
    let mut v = seed
        ^ x.cast_unsigned().wrapping_mul(0x9e37_79b9)
        ^ z.cast_unsigned().wrapping_mul(0x85eb_ca6b);
    v = (v ^ (v >> 16)).wrapping_mul(0x7feb_352d);
    v = (v ^ (v >> 15)).wrapping_mul(0x846c_a68b);
    v ^ (v >> 16)
}

fn gradient_noise(seed: u32, world_x: f32, world_z: f32) -> f32 {
    let ix = lattice_index(world_x);
    let iz = lattice_index(world_z);
    let (fraction_x, fraction_z) = (world_x - lattice_f32(ix), world_z - lattice_f32(iz));
    let fade = |t: f32| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let (smooth_x, smooth_z) = (fade(fraction_x), fade(fraction_z));
    let dot = |gx: i32, gz: i32, dx: f32, dz: f32| {
        const DIAGONAL: f32 = 0.707_106_77;
        let directions = [
            (1.0, 0.0),
            (-1.0, 0.0),
            (0.0, 1.0),
            (0.0, -1.0),
            (DIAGONAL, DIAGONAL),
            (-DIAGONAL, DIAGONAL),
            (DIAGONAL, -DIAGONAL),
            (-DIAGONAL, -DIAGONAL),
        ];
        let direction = directions[usize::try_from(hash(seed, gx, gz) & 7).unwrap_or(0)];
        direction.0 * dx + direction.1 * dz
    };
    let lower_left = dot(ix, iz, fraction_x, fraction_z);
    let lower_right = dot(ix + 1, iz, fraction_x - 1.0, fraction_z);
    let upper_left = dot(ix, iz + 1, fraction_x, fraction_z - 1.0);
    let upper_right = dot(ix + 1, iz + 1, fraction_x - 1.0, fraction_z - 1.0);
    let lower = lower_left + smooth_x * (lower_right - lower_left);
    let upper = upper_left + smooth_x * (upper_right - upper_left);
    // Gradient dot products stay within [-sqrt(2), sqrt(2)]; scale then clamp to [0,1].
    (0.5 + (lower + smooth_z * (upper - lower)) * 0.5).clamp(0.0, 1.0)
}

/// Overwrites heights with fractal noise in `0..=height`, evaluated at world coordinates.
///
/// # Errors
/// Returns `Cancelled` or `BadParameter` for invalid settings.
pub fn generate(
    grid: &mut HeightGrid,
    noise: &Noise,
    height: f32,
    cancel: &dyn Fn() -> bool,
) -> Result<(), TerrainError> {
    if !(noise.scale.is_finite()
        && noise.scale > 0.0
        && noise.octaves > 0
        && noise.octaves <= 8
        && noise.roughness.is_finite()
        && (0.0..=1.0).contains(&noise.roughness)
        && noise.lacunarity.is_finite()
        && (1.5..=3.0).contains(&noise.lacunarity)
        && noise.warp.is_finite()
        && noise.warp >= 0.0
        && height.is_finite()
        && height >= 0.0)
    {
        return Err(TerrainError::BadParameter { name: "noise".into(), why: "use scale > 0, 1..=8 octaves, roughness 0..=1, lacunarity 1.5..=3, warp >= 0 and finite nonnegative height".into() });
    }
    let shared = noise.basis.map(|basis| shared_noise(noise, basis));
    if let Some(shared) = &shared {
        shared
            .validate()
            .map_err(|error| TerrainError::BadParameter {
                name: "noise".into(),
                why: error.to_string(),
            })?;
    }
    let mut values = Vec::with_capacity(grid.width() * grid.depth());
    for j in 0..grid.depth() {
        if cancel() {
            return Err(TerrainError::Cancelled);
        }
        for i in 0..grid.width() {
            let (wx, wz) = (grid.world_x(i), grid.world_z(j));
            if let Some(shared) = &shared {
                let sample = shared.sample2(f64::from(wx), f64::from(wz));
                values.push(fraction(sample) * height);
                continue;
            }
            let (mut x, mut z) = (wx / noise.scale, wz / noise.scale);
            if noise.warp != 0.0 {
                x += (gradient_noise(
                    noise.seed ^ 0xa511_e9b3,
                    wx / noise.scale + 17.0,
                    wz / noise.scale,
                ) - 0.5)
                    * noise.warp
                    / noise.scale;
                z += (gradient_noise(
                    noise.seed ^ 0x63d8_3595,
                    wx / noise.scale,
                    wz / noise.scale + 31.0,
                ) - 0.5)
                    * noise.warp
                    / noise.scale;
            }
            let (mut freq, mut amp, mut total, mut norm) = (1.0, 1.0, 0.0, 0.0);
            for octave in 0..noise.octaves {
                let n = gradient_noise(
                    noise.seed.wrapping_add(octave.wrapping_mul(0x9e37_79b9)),
                    x * freq,
                    z * freq,
                );
                total += if noise.ridged {
                    (1.0 - (2.0 * n - 1.0).abs()).clamp(0.0, 1.0) * amp
                } else {
                    n * amp
                };
                norm += amp;
                amp *= noise.roughness;
                freq *= noise.lacunarity;
            }
            values.push((total / norm) * height);
        }
    }
    grid.heights_mut().copy_from_slice(&values);
    Ok(())
}

/// Snaps heights to levels between the grid minimum and maximum; `sharpness` 0 leaves them unchanged.
pub fn terrace(grid: &mut HeightGrid, steps: u32, sharpness: f32) {
    if steps == 0 || sharpness <= 0.0 {
        return;
    }
    let (lo, hi) = range(grid.heights());
    if hi <= lo {
        return;
    }
    let n = count_f32(usize::try_from(steps).unwrap_or(usize::MAX));
    for value in grid.heights_mut() {
        let snapped = lo + ((*value - lo) / (hi - lo) * n).round() / n * (hi - lo);
        *value += (snapped - *value) * sharpness.clamp(0.0, 1.0);
    }
}

fn range(values: &[f32]) -> (f32, f32) {
    values
        .iter()
        .fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(*v), hi.max(*v)))
}

/// Applies a 3 by 3 box blur, with edges clamped.
pub fn smooth(grid: &mut HeightGrid, passes: u32) {
    let (w, d) = (grid.width(), grid.depth());
    for _ in 0..passes {
        let source = grid.heights().to_vec();
        let mut out = vec![0.0; source.len()];
        for z in 0..d {
            for x in 0..w {
                let mut sum = 0.0;
                for dz in -1_i32..=1 {
                    for dx in -1_i32..=1 {
                        let xx = offset(x, dx, w).unwrap_or(if dx < 0 { 0 } else { w - 1 });
                        let zz = offset(z, dz, d).unwrap_or(if dz < 0 { 0 } else { d - 1 });
                        sum += source[zz * w + xx];
                    }
                }
                out[z * w + x] = sum / 9.0;
            }
        }
        grid.heights_mut().copy_from_slice(&out);
    }
}

/// Applies an island mask in world coordinates, smoothly reaching zero at `radius`.
pub fn edge_falloff(grid: &mut HeightGrid, centre: [f32; 2], radius: f32, reach: f32) {
    if radius <= 0.0 {
        return;
    }
    let inner = radius * (1.0 - reach.clamp(0.0, 1.0));
    let width = grid.width();
    for j in 0..grid.depth() {
        for i in 0..width {
            let dx = grid.world_x(i) - centre[0];
            let dz = grid.world_z(j) - centre[1];
            let distance = (dx * dx + dz * dz).sqrt();
            let t = ((distance - inner) / (radius - inner).max(f32::EPSILON)).clamp(0.0, 1.0);
            let factor = 1.0 - t * t * (3.0 - 2.0 * t);
            grid.heights_mut()[j * width + i] *= factor;
        }
    }
}

/// Carves a world-coordinate polyline, optionally flattening the channel floor.
pub fn carve_path(
    grid: &mut HeightGrid,
    points: &[[f32; 2]],
    width: f32,
    depth: f32,
    flatten: bool,
) {
    if points.len() < 2 || width <= 0.0 {
        return;
    }
    let (w, d) = (grid.width(), grid.depth());
    let mut distances = vec![f32::MAX; w * d];
    let mut floors = vec![0.0; w * d];
    for z in 0..d {
        for x in 0..w {
            let (px, pz) = (grid.world_x(x), grid.world_z(z));
            for segment in points.windows(2) {
                let (ax, az) = (segment[0][0], segment[0][1]);
                let (vx, vz) = (segment[1][0] - ax, segment[1][1] - az);
                let t = (((px - ax) * vx + (pz - az) * vz) / (vx * vx + vz * vz).max(f32::EPSILON))
                    .clamp(0.0, 1.0);
                let (qx, qz) = (ax + t * vx, az + t * vz);
                let dist = ((px - qx) * (px - qx) + (pz - qz) * (pz - qz)).sqrt();
                let k = z * w + x;
                if dist < distances[k] {
                    distances[k] = dist;
                    floors[k] = grid.heights()[k];
                }
            }
        }
    }
    let old = grid.heights().to_vec();
    let half = width * 0.5;
    for (k, h) in grid.heights_mut().iter_mut().enumerate() {
        if distances[k] <= half {
            let t = (distances[k] / half).clamp(0.0, 1.0);
            let amount = 1.0 - t * t * (3.0 - 2.0 * t);
            let target = if flatten {
                floors[k] - depth
            } else {
                old[k] - depth
            };
            *h += (target - *h) * amount;
        }
    }
}

/// Adds a hill, crater or mesa with world-space radius.
pub fn stamp(grid: &mut HeightGrid, centre: [f32; 2], radius: f32, height: f32, shape: StampShape) {
    if radius <= 0.0 {
        return;
    }
    let width = grid.width();
    for j in 0..grid.depth() {
        for i in 0..width {
            let dx = grid.world_x(i) - centre[0];
            let dz = grid.world_z(j) - centre[1];
            let r = ((dx * dx + dz * dz).sqrt() / radius).clamp(0.0, 1.0);
            let s = match shape {
                StampShape::Smooth => 1.0 - r * r * (3.0 - 2.0 * r),
                StampShape::Cone => 1.0 - r,
                StampShape::Plateau => {
                    let t = ((r - 0.55) / 0.45).clamp(0.0, 1.0);
                    1.0 - t * t * (3.0 - 2.0 * t)
                }
                StampShape::Crater => {
                    let bowl = (r / 0.65).min(1.0);
                    let rim = (1.0 - ((r - 0.82) / 0.18).abs()).clamp(0.0, 1.0);
                    -height * (1.0 - bowl * bowl) + height * 0.35 * rim
                }
            };
            grid.heights_mut()[j * width + i] += height * s;
        }
    }
}

/// Resamples from a noise-displaced lookup; lookup sampling clamps at the grid edge, providing a one-cell margin policy.
pub fn domain_warp(grid: &mut HeightGrid, seed: u32, scale: f32, strength: f32) {
    if scale <= 0.0 || strength == 0.0 {
        return;
    }
    let source = grid.heights().to_vec();
    let (w, d) = (grid.width(), grid.depth());
    for z in 0..d {
        for x in 0..w {
            let wx = grid.world_x(x);
            let wz = grid.world_z(z);
            let dx = (gradient_noise(seed, wx / scale, wz / scale) - 0.5) * strength;
            let dz = (gradient_noise(seed ^ 0x9e37_79b9, wx / scale, wz / scale) - 0.5) * strength;
            let fx = ((wx + dx - grid.origin()[0]) / grid.cell()).clamp(0.0, cell_f32(w - 1));
            let fz = ((wz + dz - grid.origin()[1]) / grid.cell()).clamp(0.0, cell_f32(d - 1));
            let (x0, z0) = (floor_index(fx), floor_index(fz));
            let (x1, z1) = ((x0 + 1).min(w - 1), (z0 + 1).min(d - 1));
            let (tx, tz) = (fx - cell_f32(x0), fz - cell_f32(z0));
            let a = source[z0 * w + x0] * (1.0 - tx) + source[z0 * w + x1] * tx;
            let b = source[z1 * w + x0] * (1.0 - tx) + source[z1 * w + x1] * tx;
            grid.heights_mut()[z * w + x] = a * (1.0 - tz) + b * tz;
        }
    }
}
