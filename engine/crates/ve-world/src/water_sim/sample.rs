//! Sampling and smooth, local forcing.
use super::WaterSim;

#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "grid coordinates are bounded before indexing"
)]
impl WaterSim {
    /// Adds a compact smooth bump on wet cells. Invalid inputs do nothing.
    pub fn disturb(&mut self, x: f32, z: f32, radius: f32, amount: f32) {
        if ![x, z, radius, amount].iter().all(|n| n.is_finite()) || radius <= 0.0 {
            return;
        }
        for j in 0..self.depth {
            for i in 0..self.width {
                let index = j * self.width + i;
                if self.depths[index] <= 0.0 {
                    continue;
                }
                let distance =
                    ((i as f32 + 0.5) * self.cell - x).hypot((j as f32 + 0.5) * self.cell - z);
                if distance < radius {
                    self.eta[index] += amount * (1.0 - (distance / radius).powi(8)).powi(2);
                }
            }
        }
    }

    /// Whether a world point lies over a wet cell.
    pub fn is_water(&self, x: f32, z: f32) -> bool {
        x.is_finite()
            && z.is_finite()
            && x >= 0.0
            && z >= 0.0
            && x < self.width as f32 * self.cell
            && z < self.depth as f32 * self.cell
            && self
                .depths
                .get(
                    ((z / self.cell) as usize).min(self.depth - 1) * self.width
                        + ((x / self.cell) as usize).min(self.width - 1),
                )
                .is_some_and(|depth| *depth > 0.0)
    }

    /// Bilinear displacement, zero outside the grid; boundary half-cells clamp to their centre.
    pub fn height(&self, x: f32, z: f32) -> f32 {
        if !x.is_finite()
            || !z.is_finite()
            || x < 0.0
            || z < 0.0
            || x >= self.width as f32 * self.cell
            || z >= self.depth as f32 * self.cell
        {
            return 0.0;
        }
        let x = (x / self.cell - 0.5).clamp(0.0, (self.width - 1) as f32);
        let z = (z / self.cell - 0.5).clamp(0.0, (self.depth - 1) as f32);
        let (i, j) = (x as usize, z as usize);
        let (u, v) = (x - i as f32, z - j as f32);
        let at = |i, j| self.eta[j * self.width + i];
        let (i1, j1) = ((i + 1).min(self.width - 1), (j + 1).min(self.depth - 1));
        (1.0 - v) * ((1.0 - u) * at(i, j) + u * at(i1, j))
            + v * ((1.0 - u) * at(i, j1) + u * at(i1, j1))
    }

    /// Unit normal from centred finite differences of the interpolated height.
    pub fn normal(&self, x: f32, z: f32) -> [f32; 3] {
        let h = self.cell * 0.5;
        let dx = (self.height(x + h, z) - self.height(x - h, z)) / self.cell;
        let dz = (self.height(x, z + h) - self.height(x, z - h)) / self.cell;
        let length = dx.hypot(dz).hypot(1.0);
        [-dx / length, 1.0 / length, -dz / length]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::water_sim::SimOptions;

    #[test]
    fn wet_cell_queries_clamp_roundoff_at_the_last_cell() -> Result<(), crate::water_sim::SimError>
    {
        let sim = WaterSim::new(
            &[1.0; 9],
            3,
            3,
            1.1,
            SimOptions {
                gravity: 9.81,
                damping: 0.0,
                absorb_edges: false,
            },
        )?;
        // Below the extent (3 * 1.1), but division rounds up to cell index 3.
        assert!(sim.is_water(3.3, 3.3));
        assert!(!sim.is_water(3.4, 3.3));
        assert!(!sim.is_water(f32::NAN, 0.0));
        assert!(!sim.is_water(0.0, f32::INFINITY));
        Ok(())
    }
}
