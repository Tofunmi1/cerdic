//! Validated build configuration.

use crate::error::NavError;

/// How a level is turned into a walkable grid.
///
/// Validated on construction: [`NavConfig::new`] rejects non-finite or nonsensical values, so the
/// builder never has to re-check them. The bounds are the `x`/`z` window that is voxelised; `y` is
/// accepted so callers can pass a world box unchanged, and is not used for lookup.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NavConfig {
    cell_size: f32,
    agent_radius: f32,
    agent_height: f32,
    max_step: f32,
    max_slope_deg: f32,
    bounds_min: [f32; 3],
    bounds_max: [f32; 3],
}

impl NavConfig {
    /// A configuration for `bounds` (`[min, max]` corners, metres).
    ///
    /// `max_slope_deg` is the steepest surface an agent can stand on, and `max_step` is the tallest
    /// ledge it can climb. `cell_size` sets the sampling resolution: smaller is more accurate and
    /// uses more memory.
    ///
    /// # Errors
    /// Returns a [`NavError`] for a non-finite value, a non-positive size or height, a negative
    /// radius or step, a slope outside `(0, 90)` degrees, or empty/inverted bounds.
    pub fn new(
        cell_size: f32,
        agent_radius: f32,
        agent_height: f32,
        max_step: f32,
        max_slope_deg: f32,
        bounds_min: [f32; 3],
        bounds_max: [f32; 3],
    ) -> Result<Self, NavError> {
        finite("cell size", cell_size)?;
        finite("agent radius", agent_radius)?;
        finite("agent height", agent_height)?;
        finite("max step", max_step)?;
        finite("max slope", max_slope_deg)?;
        for (field, value) in [
            ("bounds min x", bounds_min[0]),
            ("bounds min y", bounds_min[1]),
            ("bounds min z", bounds_min[2]),
            ("bounds max x", bounds_max[0]),
            ("bounds max y", bounds_max[1]),
            ("bounds max z", bounds_max[2]),
        ] {
            finite(field, value)?;
        }
        if cell_size <= 0.0 {
            return Err(NavError::BadCellSize(cell_size));
        }
        if agent_radius < 0.0 {
            return Err(NavError::BadAgentRadius(agent_radius));
        }
        if agent_height <= 0.0 {
            return Err(NavError::BadAgentHeight(agent_height));
        }
        if max_step < 0.0 {
            return Err(NavError::BadMaxStep(max_step));
        }
        if !(0.0 < max_slope_deg && max_slope_deg < 90.0) {
            return Err(NavError::BadMaxSlope(max_slope_deg));
        }
        if bounds_max[0] <= bounds_min[0] || bounds_max[2] <= bounds_min[2] {
            return Err(NavError::BadBounds {
                min: bounds_min,
                max: bounds_max,
            });
        }
        Ok(Self {
            cell_size,
            agent_radius,
            agent_height,
            max_step,
            max_slope_deg,
            bounds_min,
            bounds_max,
        })
    }

    /// Width of one cell, in metres.
    #[must_use]
    pub fn cell_size(&self) -> f32 {
        self.cell_size
    }

    /// Radius kept clear around an agent, in metres.
    #[must_use]
    pub fn agent_radius(&self) -> f32 {
        self.agent_radius
    }

    /// Height an agent needs above its feet, in metres.
    #[must_use]
    pub fn agent_height(&self) -> f32 {
        self.agent_height
    }

    /// Tallest ledge an agent can climb, in metres.
    #[must_use]
    pub fn max_step(&self) -> f32 {
        self.max_step
    }

    /// Steepest walkable surface, in degrees.
    #[must_use]
    pub fn max_slope_deg(&self) -> f32 {
        self.max_slope_deg
    }

    /// Lower corner of the voxelised window.
    #[must_use]
    pub fn bounds_min(&self) -> [f32; 3] {
        self.bounds_min
    }

    /// Upper corner of the voxelised window.
    #[must_use]
    pub fn bounds_max(&self) -> [f32; 3] {
        self.bounds_max
    }

    /// Number of cells along `x` and `z`.
    pub(crate) fn grid_dims(&self) -> (usize, usize) {
        let x = (self.bounds_max[0] - self.bounds_min[0]) / self.cell_size;
        let z = (self.bounds_max[2] - self.bounds_min[2]) / self.cell_size;
        (ceil_cells(x), ceil_cells(z))
    }

    /// World `x`/`z` of the `(0, 0)` cell corner.
    pub(crate) fn origin(&self) -> [f32; 2] {
        [self.bounds_min[0], self.bounds_min[2]]
    }
}

impl Default for NavConfig {
    fn default() -> Self {
        Self {
            cell_size: 0.3,
            agent_radius: 0.35,
            agent_height: 1.8,
            max_step: 0.4,
            max_slope_deg: 45.0,
            bounds_min: [-1.0, -1.0, -1.0],
            bounds_max: [1.0, 3.0, 1.0],
        }
    }
}

fn finite(field: &'static str, value: f32) -> Result<(), NavError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(NavError::NotFinite { field, value })
    }
}

/// Round up to a whole number of cells, never zero, so a tiny bounds still has one cell.
fn ceil_cells(value: f32) -> usize {
    let rounded = value.ceil();
    if rounded < 1.0 { 1 } else { rounded as usize }
}
