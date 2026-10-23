//! A scalar field the samplers read: heights, density masks.

/// A row-major grid of values positioned in the world (`index = z * width + x`, cell (0,0) at `origin`).
#[derive(Debug, Clone, Copy)]
pub struct Field2<'a> {
    /// The values.
    pub values: &'a [f32],
    /// Cells along x.
    pub width: usize,
    /// Cells along z.
    pub depth: usize,
    /// Metres between cells.
    pub cell: f32,
    /// World position of cell (0,0).
    pub origin: [f32; 2],
}

impl Field2<'_> {
    /// Bilinear value at WORLD `x`, `z`, clamped at the edge; 0 for an empty or mis-sized field.
    #[must_use]
    pub fn sample(&self, x: f32, z: f32) -> f32 {
        if self.width < 2
            || self.depth < 2
            || self.values.len() != self.width * self.depth
            || self.cell <= 0.0
        {
            return 0.0;
        }
        let fx =
            ((x - self.origin[0]) / self.cell).clamp(0.0, crate::num::count_f32(self.width - 1));
        let fz =
            ((z - self.origin[1]) / self.cell).clamp(0.0, crate::num::count_f32(self.depth - 1));
        let (i0, j0) = (crate::num::floor_index(fx), crate::num::floor_index(fz));
        let (i1, j1) = ((i0 + 1).min(self.width - 1), (j0 + 1).min(self.depth - 1));
        let (tx, tz) = (
            fx - crate::num::count_f32(i0),
            fz - crate::num::count_f32(j0),
        );
        let at = |i: usize, j: usize| self.values[j * self.width + i];
        let top = at(i0, j0) * (1.0 - tx) + at(i1, j0) * tx;
        let bottom = at(i0, j1) * (1.0 - tx) + at(i1, j1) * tx;
        top * (1.0 - tz) + bottom * tz
    }
}
