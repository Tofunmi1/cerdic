//! Two-scale erosion for tiled worlds.
use super::{
    ErosionError, Flow, Grid, Hydraulic, Hydrology, StreamPower, Thermal, hydraulic, stream_power,
    thermal,
};
use crate::num::{cell_f32, floor_index};

/// Extra cells simulated around a tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Halo {
    /// Cells on each side.
    pub cells: usize,
}
/// Coarse heights that constrain a refined tile.
#[derive(Debug, Clone, Copy)]
pub struct Constraint<'a> {
    /// Coarse heights in row-major order.
    pub coarse_heights: &'a [f32],
    /// Width.
    pub coarse_width: usize,
    /// Depth.
    pub coarse_depth: usize,
    /// Coarse cell spacing.
    pub coarse_cell: f32,
    /// World position of coarse cell (0,0).
    pub origin: [f32; 2],
    /// Pull strength, 0..=1.
    pub strength: f32,
}
/// Runs stream power over the coarse world and returns its flow.
///
/// # Errors
/// Returns grid, cancellation, or parameter errors from stream power and flow routing.
pub fn coarse_world(
    grid: &mut Grid<'_>,
    params: &StreamPower,
    cancel: &dyn Fn() -> bool,
) -> Result<Flow, ErosionError> {
    grid.check()?;
    stream_power(grid, None, params, cancel)?;
    super::route_flow(grid, cancel)
}
/// Refines the tile, then restores its broad shape from the coarse constraint.
///
/// # Errors
/// Returns grid, cancellation, or invalid constraint and erosion parameter errors.
pub fn refine_tile(
    tile: &mut Grid<'_>,
    tile_origin: [f32; 2],
    halo: &Halo,
    constraint: &Constraint<'_>,
    thermal_params: &Thermal,
    hydraulic_params: &Hydraulic,
    cancel: &dyn Fn() -> bool,
) -> Result<Hydrology, ErosionError> {
    tile.check()?;
    if constraint.coarse_width < 2
        || constraint.coarse_depth < 2
        || constraint.coarse_heights.len() != constraint.coarse_width * constraint.coarse_depth
        || !constraint.coarse_cell.is_finite()
        || constraint.coarse_cell <= 0.0
        || !constraint.strength.is_finite()
        || !(0.0..=1.0).contains(&constraint.strength)
    {
        return Err(ErosionError::BadParameter {
            name: "constraint",
            why: "invalid coarse grid or strength".to_owned(),
        });
    }
    thermal(tile, thermal_params, cancel)?;
    let hydro = hydraulic(tile, hydraulic_params, cancel)?;
    for z in 0..tile.depth {
        if cancel() {
            return Err(ErosionError::Cancelled);
        }
        for x in 0..tile.width {
            let wx = tile_origin[0] + cell_f32(x) * tile.cell;
            let wz = tile_origin[1] + cell_f32(z) * tile.cell;
            let gx = ((wx - constraint.origin[0]) / constraint.coarse_cell)
                .clamp(0.0, cell_f32(constraint.coarse_width - 1));
            let gz = ((wz - constraint.origin[1]) / constraint.coarse_cell)
                .clamp(0.0, cell_f32(constraint.coarse_depth - 1));
            let x0 = floor_index(gx);
            let z0 = floor_index(gz);
            let x1 = (x0 + 1).min(constraint.coarse_width - 1);
            let z1 = (z0 + 1).min(constraint.coarse_depth - 1);
            let tx = gx - cell_f32(x0);
            let tz = gz - cell_f32(z0);
            let a = constraint.coarse_heights[z0 * constraint.coarse_width + x0] * (1.0 - tx)
                + constraint.coarse_heights[z0 * constraint.coarse_width + x1] * tx;
            let b = constraint.coarse_heights[z1 * constraint.coarse_width + x0] * (1.0 - tx)
                + constraint.coarse_heights[z1 * constraint.coarse_width + x1] * tx;
            let target = a * (1.0 - tz) + b * tz;
            let i = z * tile.width + x;
            let edge_distance = x.min(z).min(tile.width - 1 - x).min(tile.depth - 1 - z);
            let blend = if halo.cells == 0 {
                constraint.strength
            } else {
                let t = (cell_f32(edge_distance) / cell_f32(halo.cells)).clamp(0.0, 1.0);
                1.0 + (constraint.strength - 1.0) * t
            };
            tile.heights[i] = (tile.heights[i] * (1.0 - blend) + target * blend).max(0.0);
        }
    }
    Ok(hydro)
}
/// Returns a conservative neighborhood reach: thermal and hydraulic iterations each move influence one cell.
#[must_use]
pub fn halo_for(thermal: &Thermal, hydraulic: &Hydraulic) -> Halo {
    Halo {
        cells: thermal
            .iterations
            .saturating_add(hydraulic.iterations)
            .min(96)
            .try_into()
            .unwrap_or(96),
    }
}
