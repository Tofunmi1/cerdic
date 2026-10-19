//! The walkable grid: a height per cell plus queries over it.

use glam::Vec2;

/// A uniform grid of cells, each either walkable at a floor height or blocked.
///
/// Cells outside the grid count as walkable while eroding, because the bounds are a window onto a
/// larger level: shrinking a floor that fills the bounds by the agent radius would be wrong.
#[derive(Clone, Debug)]
pub(crate) struct Grid {
    nx: usize,
    nz: usize,
    origin: [f32; 2],
    cell: f32,
    height: Vec<f32>,
    walkable: Vec<bool>,
}

impl Grid {
    /// Builds a grid from per-cell floor heights and walkability bits.
    pub(crate) fn new(
        nx: usize,
        nz: usize,
        origin: [f32; 2],
        cell: f32,
        height: Vec<f32>,
        walkable: Vec<bool>,
    ) -> Self {
        Self {
            nx,
            nz,
            origin,
            cell,
            height,
            walkable,
        }
    }

    /// Cells along `x`.
    pub(crate) fn nx(&self) -> usize {
        self.nx
    }

    /// Cells along `z`.
    pub(crate) fn nz(&self) -> usize {
        self.nz
    }

    /// Cell width in metres.
    pub(crate) fn cell(&self) -> f32 {
        self.cell
    }

    /// World `x`/`z` of the `(0, 0)` corner.
    pub(crate) fn origin(&self) -> [f32; 2] {
        self.origin
    }

    /// Linear index of a cell.
    pub(crate) fn index(&self, ix: usize, iz: usize) -> usize {
        iz * self.nx + ix
    }

    /// Cell coordinates of a linear index.
    pub(crate) fn coords(&self, index: usize) -> (usize, usize) {
        (index % self.nx, index / self.nx)
    }

    /// Whether a cell can be stood on.
    pub(crate) fn is_walkable(&self, index: usize) -> bool {
        self.walkable.get(index).copied().unwrap_or(false)
    }

    /// Floor height of a cell (meaningless when the cell is blocked).
    pub(crate) fn height(&self, index: usize) -> f32 {
        self.height.get(index).copied().unwrap_or(0.0)
    }

    /// World centre of a cell, in the ground plane.
    pub(crate) fn center(&self, ix: usize, iz: usize) -> Vec2 {
        let half = self.cell * 0.5;
        Vec2::new(
            self.origin[0] + ix as f32 * self.cell + half,
            self.origin[1] + iz as f32 * self.cell + half,
        )
    }

    /// World centre of a cell, with its floor height.
    pub(crate) fn center3(&self, ix: usize, iz: usize) -> [f32; 3] {
        let c = self.center(ix, iz);
        [c.x, self.height(self.index(ix, iz)), c.y]
    }

    /// The cell containing a ground-plane point, if it is inside the grid.
    pub(crate) fn cell_at(&self, p: Vec2) -> Option<(usize, usize)> {
        let fx = (p.x - self.origin[0]) / self.cell;
        let fz = (p.y - self.origin[1]) / self.cell;
        if fx < 0.0 || fz < 0.0 {
            return None;
        }
        let ix = fx.floor() as usize;
        let iz = fz.floor() as usize;
        (ix < self.nx && iz < self.nz).then_some((ix, iz))
    }

    /// Whether a ground-plane point is inside a walkable cell.
    pub(crate) fn is_walkable_at(&self, p: Vec2) -> bool {
        self.cell_at(p)
            .is_some_and(|(ix, iz)| self.is_walkable(self.index(ix, iz)))
    }

    /// Floor height at a point, when it is inside a walkable cell.
    pub(crate) fn height_at(&self, p: [f32; 3]) -> Option<f32> {
        self.cell_at(Vec2::new(p[0], p[2]))
            .filter(|&(ix, iz)| self.is_walkable(self.index(ix, iz)))
            .map(|(ix, iz)| self.height(self.index(ix, iz)))
    }

    /// The walkable cell nearest `point`, with its centre, by growing rings outward.
    ///
    /// Deterministic: the first ring that contains a walkable cell wins, and ties inside that ring
    /// are broken by the lower linear index.
    pub(crate) fn nearest_walkable(&self, point: [f32; 3]) -> Option<(usize, [f32; 3])> {
        let p = Vec2::new(point[0], point[2]);
        let cx = ((p.x - self.origin[0]) / self.cell).floor();
        let cz = ((p.y - self.origin[1]) / self.cell).floor();
        let cx = cx.clamp(0.0, self.nx.saturating_sub(1) as f32);
        let cz = cz.clamp(0.0, self.nz.saturating_sub(1) as f32);
        let cx = cx as isize;
        let cz = cz as isize;
        let max_ring = self.nx.max(self.nz).cast_signed();
        for ring in 0..=max_ring {
            let mut best: Option<(f32, usize, [f32; 3])> = None;
            for dz in -ring..=ring {
                for dx in -ring..=ring {
                    if dx.abs().max(dz.abs()) != ring {
                        continue;
                    }
                    let ix = cx + dx;
                    let iz = cz + dz;
                    if ix < 0 || iz < 0 || ix as usize >= self.nx || iz as usize >= self.nz {
                        continue;
                    }
                    let index = self.index(ix as usize, iz as usize);
                    if !self.is_walkable(index) {
                        continue;
                    }
                    let centre = self.center3(ix as usize, iz as usize);
                    let d = distance_sq(point, centre);
                    let better = best.is_none_or(|(bd, bi, _)| {
                        d < bd || (d.to_bits() == bd.to_bits() && index < bi)
                    });
                    if better {
                        best = Some((d, index, centre));
                    }
                }
            }
            if let Some((_, index, centre)) = best {
                return Some((index, centre));
            }
        }
        None
    }

    /// Removes walkable cells that have a blocked cell within `radius` metres.
    ///
    /// This is what stops an agent clipping a wall, and what closes a gap narrower than twice the
    /// radius. Out-of-grid neighbours are treated as open (see the type docs).
    pub(crate) fn erode(&mut self, radius: f32) {
        if radius <= 0.0 {
            return;
        }
        let reach = (radius / self.cell).ceil().max(1.0) as isize;
        let r2 = radius * radius;
        let cell2 = self.cell * self.cell;
        let mut out = self.walkable.clone();
        for iz in 0..self.nz {
            for ix in 0..self.nx {
                if !self.walkable[self.index(ix, iz)] {
                    continue;
                }
                let mut blocked = false;
                'around: for dz in -reach..=reach {
                    for dx in -reach..=reach {
                        if (dx * dx + dz * dz) as f32 * cell2 > r2 {
                            continue;
                        }
                        let nx = ix.cast_signed() + dx;
                        let nz = iz.cast_signed() + dz;
                        if nx < 0 || nz < 0 || nx as usize >= self.nx || nz as usize >= self.nz {
                            continue;
                        }
                        if !self.walkable[self.index(nx as usize, nz as usize)] {
                            blocked = true;
                            break 'around;
                        }
                    }
                }
                if blocked {
                    out[self.index(ix, iz)] = false;
                }
            }
        }
        self.walkable = out;
    }

    /// Number of cells an agent can stand on.
    pub(crate) fn walkable_count(&self) -> usize {
        self.walkable.iter().filter(|w| **w).count()
    }
}

fn distance_sq(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    dx * dx + dy * dy + dz * dz
}
