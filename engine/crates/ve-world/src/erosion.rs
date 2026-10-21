//! Erosion algorithms on a plain height slice (ADR 0056): flow routing, stream power, thermal, hydraulic, and two-scale tile refinement.
//!
//! The functions take `Grid`, a borrowed row-major slice with its size, so this crate depends on nothing in the engine.
mod error;
mod flow;
mod hydraulic;
mod stream;
mod thermal;
mod tile;

pub use error::ErosionError;
pub use flow::{Flow, river_mask, route_flow};
pub use hydraulic::{Hydraulic, Hydrology, hydraulic};
pub use stream::{StreamPower, stream_power};
pub use thermal::{Thermal, thermal};
pub use tile::{Constraint, Halo, coarse_world, halo_for, refine_tile};

/// The most cells along one side.
pub const MAX_SIDE: usize = 2048;

/// A borrowed height slice: row-major, `index = z * width + x`, `cell` metres between cells.
pub struct Grid<'a> {
    /// Heights in metres.
    pub heights: &'a mut [f32],
    /// Cells along x.
    pub width: usize,
    /// Cells along z.
    pub depth: usize,
    /// Metres between cells.
    pub cell: f32,
}

impl Grid<'_> {
    /// Checks the slice length and the size limits.
    ///
    /// # Errors
    /// `BadShape` for a wrong length or a side under 4, `TooLarge` over [`MAX_SIDE`].
    pub fn check(&self) -> Result<(), ErosionError> {
        if self.width < 4 || self.depth < 4 || self.heights.len() != self.width * self.depth {
            return Err(ErosionError::BadShape {
                width: self.width,
                depth: self.depth,
                len: self.heights.len(),
            });
        }
        if self.width > MAX_SIDE || self.depth > MAX_SIDE {
            return Err(ErosionError::TooLarge {
                cells: self.width.max(self.depth),
                max: MAX_SIDE,
            });
        }
        Ok(())
    }
}
