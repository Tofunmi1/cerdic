//! Turning a triangle soup into a walkable grid.
//!
//! The model is deliberately single-level and deterministic:
//!
//! 1. Each triangle is rasterised into every cell its ground-plane footprint overlaps.
//! 2. A surface no steeper than `max_slope` becomes a candidate floor in those cells. A steeper
//!    triangle is either a step riser (shorter than `max_step`, so it is ignored and the step is
//!    handled by the graph) or a wall (taller than `max_step`, so its cells are blocked).
//! 3. In each cell the lowest candidate with at least `agent_height` of clear space above it is the
//!    floor. This is what makes a low ceiling, or the underside of a table, block an agent.
//! 4. Walkable cells are eroded by `agent_radius`.
//!
//! Recasting all of this as spans, as Recast does, would support overlapping floors. Generated
//! levels here are single-storey, so a sorted height list per cell buys the same behaviour with
//! much less code; multi-level support is the obvious next step.

mod raster;

use crate::config::NavConfig;
use crate::error::NavError;
use crate::grid::Grid;
use glam::{Vec2, Vec3};

use raster::{Tri2, overlaps, plane_height};

/// Largest number of cells a mesh may use. Two million cells is about 8 MiB of state and keeps a
/// bad configuration from turning into an out-of-memory abort on wasm.
pub const MAX_CELLS: usize = 2_000_000;

/// Rasterise `vertices`/`indices` into a walkable [`Grid`].
pub(crate) fn grid_from_mesh(
    vertices: &[[f32; 3]],
    indices: &[u32],
    cfg: &NavConfig,
) -> Result<Grid, NavError> {
    let (nx, nz) = cfg.grid_dims();
    let cells = cell_count(nx, nz)?;
    let (mut samples, wall) = rasterise(vertices, indices, cfg, nx, nz);
    samples.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let (height, walkable) = floors(&samples, &wall, cells, cfg.agent_height());
    let [ox, oz] = cfg.origin();
    let mut grid = Grid::new(nx, nz, [ox, oz], cfg.cell_size(), height, walkable);
    grid.erode(cfg.agent_radius());
    Ok(grid)
}

/// Rejects configurations whose grid is too large to build.
fn cell_count(nx: usize, nz: usize) -> Result<usize, NavError> {
    let cells = nx.checked_mul(nz).ok_or(NavError::TooManyCells {
        cells: usize::MAX,
        max: MAX_CELLS,
    })?;
    if cells > MAX_CELLS {
        return Err(NavError::TooManyCells {
            cells,
            max: MAX_CELLS,
        });
    }
    Ok(cells)
}

/// Collects floor candidates and wall cells from every triangle.
fn rasterise(
    vertices: &[[f32; 3]],
    indices: &[u32],
    cfg: &NavConfig,
    nx: usize,
    nz: usize,
) -> (Vec<(u32, f32)>, Vec<bool>) {
    let mut raster = Raster {
        vertices,
        cell: cfg.cell_size(),
        origin: cfg.origin(),
        nx,
        nz,
        slope_cos: cfg.max_slope_deg().to_radians().cos(),
        max_step: cfg.max_step(),
        // (cell index, floor height) samples; sorting later makes grouping order-independent.
        samples: Vec::new(),
        wall: vec![false; nx * nz],
    };
    for triangle in indices.chunks_exact(3) {
        raster.paint(triangle);
    }
    (raster.samples, raster.wall)
}

/// Picks the lowest floor per cell that has enough headroom above it.
fn floors(
    samples: &[(u32, f32)],
    wall: &[bool],
    cells: usize,
    agent_height: f32,
) -> (Vec<f32>, Vec<bool>) {
    let mut height = vec![0.0_f32; cells];
    let mut walkable = vec![false; cells];
    let mut i = 0;
    while i < samples.len() {
        let index = samples[i].0 as usize;
        let mut unique: Vec<f32> = Vec::new();
        while i < samples.len() && samples[i].0 as usize == index {
            let h = samples[i].1;
            if unique.last().is_none_or(|last| (h - last).abs() > 1.0e-3) {
                unique.push(h);
            }
            i += 1;
        }
        if wall[index] {
            continue;
        }
        for k in 0..unique.len() {
            let clearance = if k + 1 < unique.len() {
                unique[k + 1] - unique[k]
            } else {
                f32::INFINITY
            };
            if clearance >= agent_height {
                height[index] = unique[k];
                walkable[index] = true;
                break;
            }
        }
    }
    (height, walkable)
}

/// The rasterisation state shared by every triangle.
struct Raster<'a> {
    vertices: &'a [[f32; 3]],
    cell: f32,
    origin: [f32; 2],
    nx: usize,
    nz: usize,
    slope_cos: f32,
    max_step: f32,
    samples: Vec<(u32, f32)>,
    wall: Vec<bool>,
}

impl Raster<'_> {
    /// Rasterises one triangle into the cells its footprint overlaps.
    fn paint(&mut self, triangle: &[u32]) {
        let [a, b, c] = match triangle {
            [a, b, c] => [*a, *b, *c],
            _ => return,
        };
        let tri3 = [
            self.vertices[a as usize],
            self.vertices[b as usize],
            self.vertices[c as usize],
        ];
        let v0 = Vec3::from(tri3[0]);
        let v1 = Vec3::from(tri3[1]);
        let v2 = Vec3::from(tri3[2]);
        let normal = (v1 - v0).cross(v2 - v0);
        let length = normal.length();
        if length <= 1.0e-9 {
            return;
        }
        let normal = normal / length;
        let walkable_slope = normal.y.abs() >= self.slope_cos;
        let low = tri3.iter().map(|v| v[1]).fold(f32::INFINITY, f32::min);
        let high = tri3.iter().map(|v| v[1]).fold(f32::NEG_INFINITY, f32::max);
        let tall = high - low > self.max_step;
        let tri2: Tri2 = [
            Vec2::new(tri3[0][0], tri3[0][2]),
            Vec2::new(tri3[1][0], tri3[1][2]),
            Vec2::new(tri3[2][0], tri3[2][2]),
        ];
        let normal = normal.to_array();

        let min_x = tri2.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
        let max_x = tri2.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
        let min_z = tri2.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
        let max_z = tri2.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
        let ix0 = to_index(min_x, self.origin[0], self.cell, self.nx);
        let ix1 = to_index(max_x, self.origin[0], self.cell, self.nx);
        let iz0 = to_index(min_z, self.origin[1], self.cell, self.nz);
        let iz1 = to_index(max_z, self.origin[1], self.cell, self.nz);

        for iz in iz0..=iz1 {
            for ix in ix0..=ix1 {
                let min = Vec2::new(
                    self.origin[0] + ix as f32 * self.cell,
                    self.origin[1] + iz as f32 * self.cell,
                );
                let max = min + Vec2::splat(self.cell);
                if !overlaps(&tri2, min, max) {
                    continue;
                }
                let index = iz * self.nx + ix;
                let centre = (min + max) * 0.5;
                if walkable_slope {
                    if let Some(height) = plane_height(&tri3, normal, centre) {
                        self.samples.push((index as u32, height));
                    }
                } else if tall {
                    self.wall[index] = true;
                }
            }
        }
    }
}

/// Clamp a world coordinate to a cell index, keeping the extremes inside the grid.
fn to_index(value: f32, origin: f32, cell: f32, count: usize) -> usize {
    let raw = ((value - origin) / cell).floor();
    let clamped = raw.clamp(0.0, count.saturating_sub(1) as f32);
    clamped as usize
}
