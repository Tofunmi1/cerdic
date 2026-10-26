//! Host-supplied height and surface samples for rolling terrain.

/// A regular grid of ground heights and surface identifiers in the XZ plane.
#[derive(Clone, Debug)]
pub struct GroundField {
    origin: [f64; 2],
    cell: f64,
    nx: usize,
    nz: usize,
    heights: Vec<f32>,
    surfaces: Vec<u8>,
}

impl GroundField {
    /// Construct a row-major grid. NaN height samples denote voids.
    ///
    /// # Errors
    /// Returns an error for invalid dimensions, sample counts, origin, or cell size.
    pub fn new(
        origin: [f64; 2],
        cell: f64,
        nx: usize,
        nz: usize,
        heights: Vec<f32>,
        surfaces: Vec<u8>,
    ) -> Result<Self, String> {
        if nx < 2
            || nz < 2
            || nx.checked_mul(nz) != Some(heights.len())
            || heights.len() != surfaces.len()
        {
            return Err("ground samples must fill a grid of at least 2 by 2".into());
        }
        if !cell.is_finite() || cell <= 0.0 || origin.iter().any(|v| !v.is_finite()) {
            return Err("ground needs a finite origin and positive finite cell".into());
        }
        Ok(Self {
            origin,
            cell,
            nx,
            nz,
            heights,
            surfaces,
        })
    }

    /// Bilinear ground height, or none outside the grid or over a void.
    #[must_use]
    pub fn height(&self, x: f64, z: f64) -> Option<f64> {
        let (heights, u, v) = self.samples(x, z)?;
        Some(
            (heights[0] * (1.0 - u) + heights[1] * u) * (1.0 - v)
                + (heights[2] * (1.0 - u) + heights[3] * u) * v,
        )
    }

    /// Upward unit normal from the bilinear height gradient; up where there is no ground.
    #[must_use]
    pub fn normal(&self, x: f64, z: f64) -> [f64; 3] {
        let Some((heights, u, v)) = self.samples(x, z) else {
            return [0.0, 1.0, 0.0];
        };
        let dx =
            ((heights[1] - heights[0]) * (1.0 - v) + (heights[3] - heights[2]) * v) / self.cell;
        let dz =
            ((heights[2] - heights[0]) * (1.0 - u) + (heights[3] - heights[1]) * u) / self.cell;
        let length = (dx * dx + 1.0 + dz * dz).sqrt();
        [-dx / length, 1.0 / length, -dz / length]
    }

    /// Surface identifier at the nearest sample, or the default surface outside the grid.
    #[must_use]
    pub fn surface(&self, x: f64, z: f64) -> u8 {
        let Some((column, row, u, v)) = self.cell_at(x, z) else {
            return 0;
        };
        self.surfaces[(row + usize::from(v >= 0.5)) * self.nx + column + usize::from(u >= 0.5)]
    }

    fn samples(&self, x: f64, z: f64) -> Option<([f64; 4], f64, f64)> {
        let (column, row, u, v) = self.cell_at(x, z)?;
        let index = row * self.nx + column;
        let heights = [index, index + 1, index + self.nx, index + self.nx + 1]
            .map(|index| f64::from(self.heights[index]));
        heights
            .iter()
            .all(|v| v.is_finite())
            .then_some((heights, u, v))
    }

    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "finite coordinates are checked against allocated grid bounds before indexing"
    )]
    fn cell_at(&self, x: f64, z: f64) -> Option<(usize, usize, f64, f64)> {
        let u = (x - self.origin[0]) / self.cell;
        let v = (z - self.origin[1]) / self.cell;
        if !u.is_finite()
            || !v.is_finite()
            || u < 0.0
            || v < 0.0
            || u > (self.nx - 1) as f64
            || v > (self.nz - 1) as f64
        {
            return None;
        }
        let column = (u.floor() as usize).min(self.nx - 2);
        let row = (v.floor() as usize).min(self.nz - 2);
        Some((column, row, u - column as f64, v - row as f64))
    }
}
