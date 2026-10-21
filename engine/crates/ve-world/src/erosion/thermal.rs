//! Thermal (talus) erosion.
use super::{ErosionError, Grid};
use crate::num::offset;

/// Talus angle, movement fraction, and iteration count.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Thermal {
    /// Talus angle in degrees, 1..=89.
    pub talus_deg: f32,
    /// Fraction of excess transferred.
    pub rate: f32,
    /// Iterations.
    pub iterations: u32,
}

/// Relaxes slopes while conserving material.
///
/// # Errors
/// Returns shape, cancellation, or invalid parameter errors.
pub fn thermal(
    grid: &mut Grid<'_>,
    params: &Thermal,
    cancel: &dyn Fn() -> bool,
) -> Result<(), ErosionError> {
    grid.check()?;
    validate(params)?;
    let talus = talus_height(params.talus_deg);
    let mut delta = vec![0.0_f32; grid.heights.len()];
    for _ in 0..params.iterations {
        delta.fill(0.0);
        accumulate_moves(grid, params.rate, talus, cancel, &mut delta)?;
        apply_moves(grid, cancel, &delta)?;
    }
    Ok(())
}

fn validate(params: &Thermal) -> Result<(), ErosionError> {
    if !params.talus_deg.is_finite() || !(1.0..=89.0).contains(&params.talus_deg) {
        return Err(bad("talus_deg"));
    }
    if !params.rate.is_finite() || !(0.0..=1.0).contains(&params.rate) {
        return Err(bad("rate"));
    }
    Ok(())
}

fn talus_height(degrees: f32) -> f32 {
    // Rational tangent approximation over [0, pi/2); no platform trig is used.
    let radians = degrees * 0.017_453_292;
    let square = radians * radians;
    radians * (1.0 + square * (0.333_333_3 + square * (0.133_333_3 + square * 0.053_968)))
        / (1.0 - square * (0.466_666_7 - square * 0.044_444))
}

fn accumulate_moves(
    grid: &Grid<'_>,
    rate: f32,
    talus: f32,
    cancel: &dyn Fn() -> bool,
    delta: &mut [f32],
) -> Result<(), ErosionError> {
    for z in 0..grid.depth {
        if cancel() {
            return Err(ErosionError::Cancelled);
        }
        for x in 0..grid.width {
            transfer_cell(grid, x, z, rate, talus, delta);
        }
    }
    Ok(())
}

fn transfer_cell(grid: &Grid<'_>, x: usize, z: usize, rate: f32, talus: f32, delta: &mut [f32]) {
    let index = z * grid.width + x;
    for delta_z in -1_i32..=1 {
        for delta_x in -1_i32..=1 {
            if delta_x == 0 && delta_z == 0 {
                continue;
            }
            let Some(neighbor_x) = offset(x, delta_x, grid.width) else {
                continue;
            };
            let Some(neighbor_z) = offset(z, delta_z, grid.depth) else {
                continue;
            };
            let neighbor = neighbor_z * grid.width + neighbor_x;
            if neighbor <= index {
                continue;
            }
            let diagonal = delta_x != 0 && delta_z != 0;
            let distance = grid.cell
                * if diagonal {
                    std::f32::consts::SQRT_2
                } else {
                    1.0
                };
            let excess = (grid.heights[index] - grid.heights[neighbor]).abs() - talus * distance;
            if excess <= 0.0 {
                continue;
            }
            transfer_pair(index, neighbor, excess * rate * 0.25, grid.heights, delta);
        }
    }
}

fn transfer_pair(first: usize, second: usize, amount: f32, heights: &[f32], delta: &mut [f32]) {
    if heights[first] > heights[second] {
        delta[first] -= amount;
        delta[second] += amount;
    } else {
        delta[second] -= amount;
        delta[first] += amount;
    }
}

fn apply_moves(
    grid: &mut Grid<'_>,
    cancel: &dyn Fn() -> bool,
    delta: &[f32],
) -> Result<(), ErosionError> {
    for z in 0..grid.depth {
        if cancel() {
            return Err(ErosionError::Cancelled);
        }
        for x in 0..grid.width {
            let index = z * grid.width + x;
            grid.heights[index] = (grid.heights[index] + delta[index]).max(0.0);
        }
    }
    Ok(())
}

fn bad(name: &'static str) -> ErosionError {
    ErosionError::BadParameter {
        name,
        why: "must be in its documented range".to_owned(),
    }
}
