//! The height grid: heights plus named layers over a world-positioned window.
use super::TerrainError;
use crate::num::{cell_f32, floor_index, rounded_offset, signed_cell};
use std::collections::BTreeMap;

/// The most cells along one side of a tile.
pub const MAX_SIDE: usize = 2048;

/// A tile of the world. Cell (0,0) sits at world position `origin`; cell (i,j) at `origin + (i,j) * cell`.
#[derive(Debug, Clone, PartialEq)]
pub struct HeightGrid {
    width: usize,
    depth: usize,
    cell: f32,
    origin: [f32; 2],
    heights: Vec<f32>,
    layers: BTreeMap<String, Vec<f32>>,
}

impl HeightGrid {
    /// A flat grid at height 0.
    ///
    /// # Errors
    /// `TooLarge` over [`MAX_SIDE`], `BadParameter` for a side under 2 cells or a non-positive `cell`.
    pub fn new(
        width: usize,
        depth: usize,
        cell: f32,
        origin: [f32; 2],
    ) -> Result<Self, TerrainError> {
        for side in [width, depth] {
            if side > MAX_SIDE {
                return Err(TerrainError::TooLarge {
                    side,
                    max: MAX_SIDE,
                });
            }
            if side < 2 {
                return Err(TerrainError::BadParameter {
                    name: "size".into(),
                    why: format!("a grid needs at least 2 cells per side, got {side}"),
                });
            }
        }
        if cell.is_nan() || cell <= 0.0 {
            return Err(TerrainError::BadParameter {
                name: "cell".into(),
                why: format!("the cell size must be positive, got {cell}"),
            });
        }
        Ok(Self {
            width,
            depth,
            cell,
            origin,
            heights: vec![0.0; width * depth],
            layers: BTreeMap::new(),
        })
    }

    /// Cells along x.
    #[must_use]
    pub fn width(&self) -> usize {
        self.width
    }

    /// Cells along z.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Metres between cell centres.
    #[must_use]
    pub fn cell(&self) -> f32 {
        self.cell
    }

    /// World position of cell (0,0).
    #[must_use]
    pub fn origin(&self) -> [f32; 2] {
        self.origin
    }

    /// Row-major heights, index = `z * width + x`.
    #[must_use]
    pub fn heights(&self) -> &[f32] {
        &self.heights
    }

    /// Mutable heights.
    pub fn heights_mut(&mut self) -> &mut [f32] {
        &mut self.heights
    }

    /// World x of column `i`.
    #[must_use]
    pub fn world_x(&self, i: usize) -> f32 {
        self.origin[0] + cell_f32(i) * self.cell
    }

    /// World z of row `j`.
    #[must_use]
    pub fn world_z(&self, j: usize) -> f32 {
        self.origin[1] + cell_f32(j) * self.cell
    }

    /// Bilinear height at WORLD `x`, `z` in metres, clamped at the edge.
    #[must_use]
    pub fn sample(&self, x: f32, z: f32) -> f32 {
        self.bilinear(&self.heights, x, z)
    }

    /// Bilinear value of a named layer at WORLD `x`, `z`, clamped at the edge; `None` when the grid has no such layer.
    #[must_use]
    pub fn sample_layer(&self, name: &str, x: f32, z: f32) -> Option<f32> {
        self.layers
            .get(name)
            .map(|values| self.bilinear(values, x, z))
    }

    /// Bilinear interpolation of one value per grid cell.
    fn bilinear(&self, values: &[f32], x: f32, z: f32) -> f32 {
        let fx = ((x - self.origin[0]) / self.cell).clamp(0.0, cell_f32(self.width - 1));
        let fz = ((z - self.origin[1]) / self.cell).clamp(0.0, cell_f32(self.depth - 1));
        let (i0, j0) = (floor_index(fx), floor_index(fz));
        let (i1, j1) = ((i0 + 1).min(self.width - 1), (j0 + 1).min(self.depth - 1));
        let (tx, tz) = (fx - cell_f32(i0), fz - cell_f32(j0));
        let at = |i: usize, j: usize| values[j * self.width + i];
        let top = at(i0, j0) * (1.0 - tx) + at(i1, j0) * tx;
        let bottom = at(i0, j1) * (1.0 - tx) + at(i1, j1) * tx;
        top * (1.0 - tz) + bottom * tz
    }

    /// Unit surface normal at WORLD `x`, `z` from central differences one cell apart.
    #[must_use]
    pub fn normal(&self, x: f32, z: f32) -> [f32; 3] {
        let e = self.cell;
        let dx = (self.sample(x + e, z) - self.sample(x - e, z)) / (2.0 * e);
        let dz = (self.sample(x, z + e) - self.sample(x, z - e)) / (2.0 * e);
        let length = (dx * dx + 1.0 + dz * dz).sqrt();
        [-dx / length, 1.0 / length, -dz / length]
    }

    /// A named layer, if present.
    #[must_use]
    pub fn layer(&self, name: &str) -> Option<&[f32]> {
        self.layers.get(name).map(Vec::as_slice)
    }

    /// Sets (or replaces) a layer; its length must equal `width * depth`.
    ///
    /// # Errors
    /// `BadLayer` for an empty name, `BadShape` for a wrong length.
    pub fn set_layer(&mut self, name: &str, values: Vec<f32>) -> Result<(), TerrainError> {
        if name.is_empty() {
            return Err(TerrainError::BadLayer { name: name.into() });
        }
        let expected = self.width * self.depth;
        if values.len() != expected {
            return Err(TerrainError::BadShape {
                what: format!("layer {name}"),
                expected,
                got: values.len(),
            });
        }
        self.layers.insert(name.into(), values);
        Ok(())
    }

    /// Layer names, sorted.
    #[must_use]
    pub fn layer_names(&self) -> Vec<String> {
        self.layers.keys().cloned().collect()
    }

    /// A copy of a window with its layers; the origin moves accordingly.
    ///
    /// # Errors
    /// `BadShape` when the window leaves the grid or is under 2 cells.
    pub fn sub_grid(
        &self,
        x0: usize,
        z0: usize,
        width: usize,
        depth: usize,
    ) -> Result<Self, TerrainError> {
        if width < 2 || depth < 2 || x0 + width > self.width || z0 + depth > self.depth {
            return Err(TerrainError::BadShape {
                what: "window".into(),
                expected: self.width * self.depth,
                got: (x0 + width) * (z0 + depth),
            });
        }
        let mut out = Self::new(
            width,
            depth,
            self.cell,
            [self.world_x(x0), self.world_z(z0)],
        )?;
        let copy = |source: &[f32]| {
            let mut target = Vec::with_capacity(width * depth);
            for j in z0..z0 + depth {
                target.extend_from_slice(&source[j * self.width + x0..j * self.width + x0 + width]);
            }
            target
        };
        out.heights = copy(&self.heights);
        for (name, values) in &self.layers {
            out.layers.insert(name.clone(), copy(values));
        }
        Ok(out)
    }

    /// Copies the overlap of `other` (matched by world position, same cell size) into `self`, layers by name.
    ///
    /// # Errors
    /// `BadParameter` when the cell sizes differ or the grids are not aligned to the same lattice.
    pub fn paste(&mut self, other: &Self) -> Result<(), TerrainError> {
        if (self.cell - other.cell).abs() > f32::EPSILON * self.cell {
            return Err(TerrainError::BadParameter {
                name: "cell".into(),
                why: "tiles must have the same cell size to be pasted".into(),
            });
        }
        let shift = |from: f32, to: f32| (from - to) / self.cell;
        let (sx, sz) = (
            shift(other.origin[0], self.origin[0]),
            shift(other.origin[1], self.origin[1]),
        );
        if (sx - sx.round()).abs() > 1e-3 || (sz - sz.round()).abs() > 1e-3 {
            return Err(TerrainError::BadParameter {
                name: "origin".into(),
                why: "tiles must sit on the same cell lattice to be pasted".into(),
            });
        }
        let (dx, dz) = (rounded_offset(sx.round()), rounded_offset(sz.round()));
        let names: Vec<String> = other.layers.keys().cloned().collect();
        for name in &names {
            let missing = !self.layers.contains_key(name);
            if missing {
                self.layers
                    .insert(name.clone(), vec![0.0; self.width * self.depth]);
            }
        }
        for j in 0..other.depth {
            for i in 0..other.width {
                let (x, z) = (signed_cell(i) + dx, signed_cell(j) + dz);
                if x < 0 || z < 0 || x >= signed_cell(self.width) || z >= signed_cell(self.depth) {
                    continue;
                }
                let (Ok(x), Ok(z)) = (usize::try_from(x), usize::try_from(z)) else {
                    continue;
                };
                let (to, from) = (z * self.width + x, j * other.width + i);
                self.heights[to] = other.heights[from];
                for name in &names {
                    if let (Some(target), Some(source)) =
                        (self.layers.get_mut(name), other.layers.get(name))
                    {
                        target[to] = source[from];
                    }
                }
            }
        }
        Ok(())
    }
}
