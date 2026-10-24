//! The public navigation mesh: build, queries and pathfinding.

use glam::Vec2;

use crate::astar;
use crate::build;
use crate::config::NavConfig;
use crate::error::NavError;
use crate::funnel::{self, Portal};
use crate::grid::Grid;
use crate::path::Path;
use crate::raycast;
use crate::{AreaId, Edge, EdgeKind, NavSpace, NodeId};

/// A walkable representation of a level, ready for pathfinding.
///
/// Build it once from the level's triangles (with the same [`NavConfig`] the game uses for its
/// agents) and query it as often as needed. Building is deterministic: the same vertices, indices
/// and config always produce the same mesh.
#[derive(Clone, Debug)]
pub struct NavMesh {
    config: NavConfig,
    grid: Grid,
    cell_to_node: Vec<NodeId>,
    node_to_cell: Vec<usize>,
}

impl NavMesh {
    /// Voxelises `vertices`/`indices` into a walkable grid.
    ///
    /// `indices` must be a multiple of three and every index must point at a vertex. Triangles may
    /// be wound either way; the slope test uses the absolute vertical of the normal.
    ///
    /// # Errors
    /// Returns [`NavError::EmptyMesh`] when there is nothing to build, [`NavError::BadIndexCount`]
    /// or [`NavError::BadIndex`] for a malformed index list, and [`NavError::TooManyCells`] when
    /// the configured cells exceed [`build::MAX_CELLS`].
    pub fn build(
        vertices: &[[f32; 3]],
        indices: &[u32],
        config: &NavConfig,
    ) -> Result<Self, NavError> {
        if indices.is_empty() || vertices.is_empty() {
            return Err(NavError::EmptyMesh);
        }
        if !indices.len().is_multiple_of(3) {
            return Err(NavError::BadIndexCount {
                indices: indices.len(),
            });
        }
        for &index in indices {
            if index as usize >= vertices.len() {
                return Err(NavError::BadIndex {
                    index,
                    vertices: vertices.len(),
                });
            }
        }
        let grid = build::grid_from_mesh(vertices, indices, config)?;
        let cell_to_node = vec![NodeId::MAX; grid.nx() * grid.nz()];
        let mut mesh = Self {
            config: *config,
            grid,
            cell_to_node,
            node_to_cell: Vec::new(),
        };
        mesh.rebuild_node_ids();
        Ok(mesh)
    }

    /// The configuration the mesh was built with.
    #[must_use]
    pub fn config(&self) -> &NavConfig {
        &self.config
    }

    /// The underlying grid, for tests and crate-internal queries.
    #[cfg(test)]
    pub(crate) fn grid(&self) -> &Grid {
        &self.grid
    }

    /// Grid dimensions `(x, z)` in cells.
    #[must_use]
    pub fn dimensions(&self) -> (usize, usize) {
        (self.grid.nx(), self.grid.nz())
    }

    /// How many cells an agent can stand on.
    #[must_use]
    pub fn walkable_count(&self) -> usize {
        self.grid.walkable_count()
    }

    /// Paints walkable cell centres inside the inclusive axis-aligned box with an area id.
    pub fn paint_area(&mut self, min: [f32; 3], max: [f32; 3], area: AreaId) {
        for iz in 0..self.grid.nz() {
            for ix in 0..self.grid.nx() {
                let cell = self.grid.index(ix, iz);
                if self.grid.is_walkable(cell) {
                    let p = self.grid.center3(ix, iz);
                    if (0..3).all(|axis| p[axis] >= min[axis] && p[axis] <= max[axis]) {
                        self.grid.set_area(cell, area.0);
                    }
                }
            }
        }
    }

    fn rebuild_node_ids(&mut self) {
        self.cell_to_node.fill(NodeId::MAX);
        self.node_to_cell.clear();
        for cell in 0..self.grid.nx() * self.grid.nz() {
            if self.grid.is_walkable(cell) {
                let id = self.node_to_cell.len() as NodeId;
                self.cell_to_node[cell] = id;
                self.node_to_cell.push(cell);
            }
        }
    }

    fn allowed_step(&self, a: usize, b: usize, dx: isize, dz: isize) -> bool {
        if !self.grid.is_walkable(b)
            || (self.grid.height(b) - self.grid.height(a)).abs() > self.config.max_step()
        {
            return false;
        }
        if dx != 0 && dz != 0 {
            let (x, z) = self.grid.coords(a);
            self.grid
                .is_walkable(self.grid.index((x.cast_signed() + dx) as usize, z))
                && self
                    .grid
                    .is_walkable(self.grid.index(x, (z.cast_signed() + dz) as usize))
        } else {
            true
        }
    }

    /// Whether a world point is over a walkable cell.
    #[must_use]
    pub fn is_walkable(&self, point: [f32; 3]) -> bool {
        self.grid.is_walkable_at(Vec2::new(point[0], point[2]))
    }

    /// Floor height at a world point, when it is walkable.
    #[must_use]
    pub fn height_at(&self, point: [f32; 3]) -> Option<f32> {
        self.grid.height_at(point)
    }

    /// The centre of the walkable cell nearest `point`, or `None` when the mesh has none.
    #[must_use]
    pub fn nearest_walkable(&self, point: [f32; 3]) -> Option<[f32; 3]> {
        self.grid.nearest_walkable(point).map(|(_, centre)| centre)
    }

    /// Finds a smoothed path from `start` to `end`.
    ///
    /// Both ends are snapped to the nearest walkable cell. A* runs on the grid and the funnel
    /// algorithm pulls the result tight without leaving walkable cells.
    ///
    /// # Errors
    /// Returns [`NavError::NoWalkableCell`] when an end is nowhere near walkable ground, and
    /// [`NavError::NoPath`] when the two ends are blocked from each other (for example by a gap
    /// narrower than twice the agent radius).
    pub fn find_path(&self, start: [f32; 3], end: [f32; 3]) -> Result<Path, NavError> {
        let (start_index, start_centre) = self
            .grid
            .nearest_walkable(start)
            .ok_or(NavError::NoWalkableCell { at: start })?;
        let (end_index, end_centre) = self
            .grid
            .nearest_walkable(end)
            .ok_or(NavError::NoWalkableCell { at: end })?;
        let cells =
            astar::find_path_cells(&self.grid, start_index, end_index, self.config.max_step())
                .ok_or(NavError::NoPath { start, end })?;
        let portals = self.portals(&cells);
        let flat = funnel::string_pull(
            Vec2::new(start_centre[0], start_centre[2]),
            Vec2::new(end_centre[0], end_centre[2]),
            &portals,
        );
        let points = flat.into_iter().map(|p| self.point3(p)).collect();
        Ok(Path::new(self.simplify(points)))
    }

    /// The first blocked point on the segment `a -> b`, or `None` when sight is clear.
    #[must_use]
    pub fn raycast(&self, a: [f32; 3], b: [f32; 3]) -> Option<[f32; 3]> {
        raycast::raycast(&self.grid, a, b)
    }

    /// Whether `a` can see `b` without crossing blocked ground.
    #[must_use]
    pub fn line_of_sight(&self, a: [f32; 3], b: [f32; 3]) -> bool {
        self.raycast(a, b).is_none()
    }

    /// Builds the portal list between consecutive cells of an A* path.
    fn portals(&self, cells: &[u32]) -> Vec<Portal> {
        let mut portals = Vec::with_capacity(cells.len().saturating_sub(1));
        for pair in cells.windows(2) {
            let (ax, az) = self.grid.coords(pair[0] as usize);
            let (bx, bz) = self.grid.coords(pair[1] as usize);
            let from = self.grid.center(ax, az);
            let to = self.grid.center(bx, bz);
            let (p, q) = self.shared_edge(ax, az, bx, bz);
            portals.push(funnel::orient(from, to, p, q));
        }
        portals
    }

    /// The edge or corner shared by two neighbouring cells.
    fn shared_edge(&self, ax: usize, az: usize, bx: usize, bz: usize) -> (Vec2, Vec2) {
        let [ox, oz] = self.grid.origin();
        let cell = self.grid.cell();
        if ax == bx {
            let z = oz + (if bz > az { (az + 1) as f32 } else { az as f32 }) * cell;
            (
                Vec2::new(ox + ax as f32 * cell, z),
                Vec2::new(ox + (ax + 1) as f32 * cell, z),
            )
        } else if az == bz {
            let x = ox + (if bx > ax { (ax + 1) as f32 } else { ax as f32 }) * cell;
            (
                Vec2::new(x, oz + az as f32 * cell),
                Vec2::new(x, oz + (az + 1) as f32 * cell),
            )
        } else {
            let x = ox + (if bx > ax { (ax + 1) as f32 } else { ax as f32 }) * cell;
            let z = oz + (if bz > az { (az + 1) as f32 } else { az as f32 }) * cell;
            // A diagonal step shares only a corner. Give the funnel a tiny segment across it (still
            // inside the open 2x2 block the no-corner-cutting rule guarantees) so it can tighten.
            let corner = Vec2::new(x, z);
            let direction = Vec2::new(bx as f32 - ax as f32, bz as f32 - az as f32);
            let half_width = cell * 0.01;
            let across = Vec2::new(-direction.y, direction.x) * half_width;
            (corner + across, corner - across)
        }
    }

    /// Drops corners that a straight walkable line makes redundant.
    ///
    /// The funnel only sees the cells A* chose, so an L-shaped grid route can stay longer than the
    /// level allows. This greedy pass keeps the farthest visible corner from each waypoint, using
    /// the grid raycast, so the final route is short *and* stays on walkable cells.
    fn simplify(&self, points: Vec<[f32; 3]>) -> Vec<[f32; 3]> {
        if points.len() <= 2 {
            return points;
        }
        let mut out = Vec::with_capacity(points.len());
        out.push(points[0]);
        let mut i = 0;
        while i + 1 < points.len() {
            let mut j = points.len() - 1;
            while j > i + 1 && !self.line_of_sight(points[i], points[j]) {
                j -= 1;
            }
            out.push(points[j]);
            i = j;
        }
        out
    }

    /// A ground-plane point as a world point, using the floor height where possible.
    fn point3(&self, p: Vec2) -> [f32; 3] {
        let y = self
            .grid
            .height_at([p.x, 0.0, p.y])
            .or_else(|| {
                self.grid
                    .nearest_walkable([p.x, 0.0, p.y])
                    .map(|(_, centre)| centre[1])
            })
            .unwrap_or(0.0);
        [p.x, y, p.y]
    }
}

impl NavSpace for NavMesh {
    fn node_count(&self) -> usize {
        self.node_to_cell.len()
    }
    fn position(&self, node: NodeId) -> [f32; 3] {
        let (x, z) = self.grid.coords(self.node_to_cell[node as usize]);
        self.grid.center3(x, z)
    }
    fn area(&self, node: NodeId) -> AreaId {
        AreaId(self.grid.area(self.node_to_cell[node as usize]))
    }
    fn neighbours(&self, node: NodeId, out: &mut Vec<Edge>) {
        const DELTAS: [(isize, isize); 8] = [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
            (1, 1),
            (1, -1),
            (-1, 1),
            (-1, -1),
        ];
        let cell = self.node_to_cell[node as usize];
        let (x, z) = self.grid.coords(cell);
        for (dx, dz) in DELTAS {
            let nx = x.cast_signed() + dx;
            let nz = z.cast_signed() + dz;
            if nx < 0 || nz < 0 || nx as usize >= self.grid.nx() || nz as usize >= self.grid.nz() {
                continue;
            }
            let next = self.grid.index(nx as usize, nz as usize);
            if !self.allowed_step(cell, next, dx, dz) {
                continue;
            }
            let to = self.cell_to_node[next];
            let a = self.grid.center3(x, z);
            let b = self.grid.center3(nx as usize, nz as usize);
            let vx = a[0] - b[0];
            let vy = a[1] - b[1];
            let vz = a[2] - b[2];
            out.push(Edge {
                to,
                length: (vx * vx + vy * vy + vz * vz).sqrt(),
                kind: EdgeKind::WALK,
            });
        }
    }
    fn locate(&self, point: [f32; 3]) -> Option<NodeId> {
        self.grid
            .nearest_walkable(point)
            .map(|(cell, _)| self.cell_to_node[cell])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::astar;

    fn build_wall_with_gap() -> NavMesh {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut quad = |a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3]| {
            let base = vertices.len() as u32;
            vertices.extend_from_slice(&[a, b, c, d]);
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        };
        quad(
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [10.0, 0.0, 10.0],
            [0.0, 0.0, 10.0],
        );
        for (z0, z1) in [(0.0, 3.5), (6.5, 10.0)] {
            quad(
                [5.0, 0.0, z0],
                [5.0, 0.0, z1],
                [5.0, 2.5, z1],
                [5.0, 2.5, z0],
            );
        }
        let config = NavConfig::new(
            0.25,
            0.2,
            1.8,
            0.4,
            45.0,
            [0.0, -1.0, 0.0],
            [10.0, 3.0, 10.0],
        )
        .expect("valid config");
        NavMesh::build(&vertices, &indices, &config).expect("valid mesh")
    }

    #[test]
    fn funnel_is_never_longer_than_the_raw_grid_path() {
        let nav = build_wall_with_gap();
        let start = [1.0, 0.0, 1.0];
        let end = [9.0, 0.0, 1.0];
        let path = nav.find_path(start, end).expect("a route exists");
        let (start_cell, _) = nav.grid().nearest_walkable(start).expect("start cell");
        let (end_cell, _) = nav.grid().nearest_walkable(end).expect("end cell");
        let cells =
            astar::find_path_cells(nav.grid(), start_cell, end_cell, nav.config().max_step())
                .expect("A* finds the same route");
        let raw: f32 = cells
            .windows(2)
            .map(|pair| {
                let (ax, az) = nav.grid().coords(pair[0] as usize);
                let (bx, bz) = nav.grid().coords(pair[1] as usize);
                let a = nav.grid().center3(ax, az);
                let b = nav.grid().center3(bx, bz);
                let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
            })
            .sum();
        assert!(
            path.length() <= raw + 1.0e-3,
            "funnel {} longer than raw {raw}",
            path.length()
        );
    }
}
