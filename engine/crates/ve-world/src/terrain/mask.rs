//! Mask layers computed from heights.
use super::{HeightGrid, TerrainError};
use crate::num::{cell_f32, count_f32, lattice_f32, offset};

/// Which mask to compute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mask {
    /// Slope angle over 90 degrees.
    Slope,
    /// Scaled Laplacian, flat is 0.5.
    Curvature,
    /// Height normalized to the grid range.
    Height,
    /// Downhill facing direction, north at zero.
    Aspect,
    /// Eight direction horizon occlusion.
    Occlusion,
}
/// How [`combine`] merges masks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combine {
    /// a*b.
    Multiply,
    /// a+b.
    Add,
    /// maximum.
    Max,
    /// minimum.
    Min,
    /// a-b.
    Subtract,
}

/// Computes a mask. Exact interior agreement requires a 1 cell halo, or 32 cells for occlusion.
///
/// # Errors
/// Returns `Cancelled` when requested.
pub fn compute_mask(
    grid: &HeightGrid,
    mask: Mask,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<f32>, TerrainError> {
    let (w, d, cell) = (grid.width(), grid.depth(), grid.cell());
    let (lo, hi) = grid
        .heights()
        .iter()
        .fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
    let mut out = Vec::with_capacity(w * d);
    for z in 0..d {
        if cancel() {
            return Err(TerrainError::Cancelled);
        }
        for x in 0..w {
            let h = |xx: usize, zz: usize| grid.heights()[zz * w + xx];
            let xm = x.saturating_sub(1);
            let xp = (x + 1).min(w - 1);
            let zm = z.saturating_sub(1);
            let zp = (z + 1).min(d - 1);
            let dx = (h(xp, z) - h(xm, z)) / (cell_f32((xp - xm).max(1)) * cell);
            let dz = (h(x, zp) - h(x, zm)) / (cell_f32((zp - zm).max(1)) * cell);
            let v = match mask {
                Mask::Slope => {
                    atan_positive((dx * dx + dz * dz).sqrt()) / core::f32::consts::FRAC_PI_2
                }
                Mask::Curvature => {
                    let lap = (h(xp, z) - 2.0 * h(x, z) + h(xm, z) + h(x, zp) - 2.0 * h(x, z)
                        + h(x, zm))
                        / (cell * cell);
                    (0.5 + lap * cell * 0.05).clamp(0.0, 1.0)
                }
                Mask::Height => {
                    if hi > lo {
                        (h(x, z) - lo) / (hi - lo)
                    } else {
                        0.0
                    }
                }
                Mask::Aspect => {
                    if dx == 0.0 && dz == 0.0 {
                        0.0
                    } else {
                        compass(-dx, -dz)
                    }
                }
                Mask::Occlusion => occlusion(grid, x, z),
            };
            out.push(v.clamp(0.0, 1.0));
        }
    }
    Ok(out)
}

fn occlusion(grid: &HeightGrid, x: usize, z: usize) -> f32 {
    let directions: [(i32, i32); 8] = [
        (1, 0),
        (1, 1),
        (0, 1),
        (-1, 1),
        (-1, 0),
        (-1, -1),
        (0, -1),
        (1, -1),
    ];
    let center = grid.heights()[z * grid.width() + x];
    let mut blocked = 0_usize;
    for (dx, dz) in directions {
        let mut hit = false;
        for step in 1..=32_i32 {
            let Some(xx) = offset(x, dx * step, grid.width()) else {
                break;
            };
            let Some(zz) = offset(z, dz * step, grid.depth()) else {
                break;
            };
            let rise = grid.heights()[zz * grid.width() + xx] - center;
            let diagonal = dx != 0 && dz != 0;
            let direction_length = if diagonal {
                core::f32::consts::SQRT_2
            } else {
                1.0
            };
            let horizontal = direction_length * lattice_f32(step) * grid.cell();
            // tan(20 degrees) is approximated by the fixed constant 0.36397.
            if rise > horizontal * 0.363_970_2 {
                hit = true;
                break;
            }
        }
        if hit {
            blocked += 1;
        }
    }
    count_f32(blocked) / 8.0
}

/// Polynomial atan approximation: maximum absolute error is below 0.005 radians on [-1,1].
fn atan_unit(x: f32) -> f32 {
    let a = x.abs();
    let result = a * (core::f32::consts::FRAC_PI_4 + 0.273 * (1.0 - a));
    result.copysign(x)
}

fn atan_positive(x: f32) -> f32 {
    if x <= 1.0 {
        atan_unit(x)
    } else {
        core::f32::consts::FRAC_PI_2 - atan_unit(1.0 / x)
    }
}

fn compass(x: f32, z: f32) -> f32 {
    // atan2(x,z), wrapped to [0, 2pi). Quadrant reduction keeps atan input within [-1,1].
    let a = if z.abs() >= x.abs() {
        let base = atan_unit(x / z.abs());
        if z >= 0.0 {
            base
        } else {
            core::f32::consts::PI - base
        }
    } else {
        let base = core::f32::consts::FRAC_PI_2 - atan_unit(z / x.abs());
        if x >= 0.0 { base } else { -base }
    };
    (a / core::f32::consts::TAU).rem_euclid(1.0)
}

/// Maps `(v-lo)/(hi-lo)` into `[0,1]`, optionally applying smoothstep.
#[expect(
    clippy::float_cmp,
    reason = "exact equality identifies a constant input range"
)]
pub fn remap(values: &mut [f32], lo: f32, hi: f32, smooth: bool) {
    for v in values {
        let mut t = if hi == lo {
            0.0
        } else {
            ((*v - lo) / (hi - lo)).clamp(0.0, 1.0)
        };
        if smooth {
            t = t * t * (3.0 - 2.0 * t);
        }
        *v = t;
    }
}

/// Combines equal or different length slices; the shorter length wins.
#[must_use]
pub fn combine(a: &[f32], b: &[f32], op: Combine) -> Vec<f32> {
    a.iter()
        .zip(b)
        .map(|(x, y)| {
            match op {
                Combine::Multiply => x * y,
                Combine::Add => x + y,
                Combine::Max => x.max(*y),
                Combine::Min => x.min(*y),
                Combine::Subtract => x - y,
            }
            .clamp(0.0, 1.0)
        })
        .collect()
}
