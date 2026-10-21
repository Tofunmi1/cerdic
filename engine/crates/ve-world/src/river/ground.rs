//! A borrowed height slice positioned in the world.
use super::RiverError;

/// A mutable height slice: row-major (`index = z * width + x`), `cell` metres apart, cell (0,0) at world `origin`.
pub struct Ground<'a> {
    /// Heights in metres.
    pub heights: &'a mut [f32],
    /// Cells along x.
    pub width: usize,
    /// Cells along z.
    pub depth: usize,
    /// Metres between cells.
    pub cell: f32,
    /// World position of cell (0,0).
    pub origin: [f32; 2],
}

/// The same, read-only.
#[derive(Clone, Copy)]
pub struct GroundRef<'a> {
    /// Heights in metres.
    pub heights: &'a [f32],
    /// Cells along x.
    pub width: usize,
    /// Cells along z.
    pub depth: usize,
    /// Metres between cells.
    pub cell: f32,
    /// World position of cell (0,0).
    pub origin: [f32; 2],
}

pub(super) fn check(len: usize, width: usize, depth: usize, cell: f32) -> Result<(), RiverError> {
    if width < 4 || depth < 4 || len != width * depth {
        return Err(RiverError::BadShape { width, depth, len });
    }
    if !cell.is_finite() || cell <= 0.0 {
        return Err(RiverError::BadParameter {
            name: "cell",
            why: "the cell size must be positive".into(),
        });
    }
    Ok(())
}

impl Ground<'_> {
    /// Checks the slice length and the cell size.
    ///
    /// # Errors
    /// `BadShape` for a wrong length or a side under 4, `BadParameter` for a bad cell size.
    pub fn check(&self) -> Result<(), RiverError> {
        check(self.heights.len(), self.width, self.depth, self.cell)
    }
}

impl GroundRef<'_> {
    /// Checks the slice length and the cell size.
    ///
    /// # Errors
    /// `BadShape` for a wrong length or a side under 4, `BadParameter` for a bad cell size.
    pub fn check(&self) -> Result<(), RiverError> {
        check(self.heights.len(), self.width, self.depth, self.cell)
    }
}
