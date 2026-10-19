//! Mirror-symmetry error for the X, Y and Z planes.

use std::collections::HashMap;

use glam::Vec3;

use crate::error::MeasureError;
use crate::mesh::Mesh;

/// The furthest ring the grid search walks before giving up.
const MAX_RADIUS: i32 = 4096;
/// How many cells span the longest side of the bounding box.
const GRID_SPAN: f32 = 32.0;

/// Which axis a mirror plane is perpendicular to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plane {
    /// The plane `x = centre_x` (mirror left against right).
    X,
    /// The plane `y = centre_y` (mirror up against down).
    Y,
    /// The plane `z = centre_z` (mirror front against back).
    Z,
}

impl Plane {
    /// All three planes, in a fixed order (X, then Y, then Z).
    #[must_use]
    pub fn all() -> [Self; 3] {
        [Self::X, Self::Y, Self::Z]
    }

    /// The plane's axis index: 0 for X, 1 for Y, 2 for Z.
    #[must_use]
    pub fn axis(self) -> usize {
        match self {
            Self::X => 0,
            Self::Y => 1,
            Self::Z => 2,
        }
    }

    fn mirror(self, p: Vec3, centre: Vec3) -> Vec3 {
        match self {
            Self::X => Vec3::new(2.0 * centre.x - p.x, p.y, p.z),
            Self::Y => Vec3::new(p.x, 2.0 * centre.y - p.y, p.z),
            Self::Z => Vec3::new(p.x, p.y, 2.0 * centre.z - p.z),
        }
    }
}

/// How symmetric a mesh is about a plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SymmetryReport {
    /// The plane tested.
    pub plane: Plane,
    /// The largest distance from a vertex to the nearest mirrored vertex, in metres.
    pub max: f32,
    /// The mean of those distances, in metres.
    pub mean: f32,
}

/// Measure how far a mesh is from mirror symmetry about the plane through its own centre.
///
/// For every vertex the nearest vertex to its mirror image is found through a spatial grid, and
/// `max`/`mean` are taken over those distances. A perfectly symmetric mesh gives zero.
///
/// # Errors
/// Returns [`MeasureError`] when the mesh is malformed or empty.
pub fn symmetry_error(mesh: &Mesh, plane: Plane) -> Result<SymmetryReport, MeasureError> {
    mesh.validate()?;
    let centre = mesh.bounds().centre_vec();
    let grid = Grid::build(mesh);
    let mut max = 0.0f32;
    let mut sum = 0.0f32;
    for p in mesh.positions {
        let mirrored = plane.mirror(Vec3::from_array(*p), centre);
        if let Some(distance) = grid.nearest(mirrored) {
            max = max.max(distance);
            sum += distance;
        }
    }
    let mean = sum / mesh.positions.len() as f32;
    Ok(SymmetryReport { plane, max, mean })
}

/// The plane (X, Y or Z) about which the mesh is most symmetric, with its report.
///
/// # Errors
/// Returns [`MeasureError`] when the mesh is malformed or empty.
pub fn best_symmetry_plane(mesh: &Mesh) -> Result<SymmetryReport, MeasureError> {
    let mut best: Option<SymmetryReport> = None;
    for plane in Plane::all() {
        let report = symmetry_error(mesh, plane)?;
        if best.is_none_or(|b| report.max < b.max) {
            best = Some(report);
        }
    }
    best.ok_or(MeasureError::Empty)
}

/// A uniform grid over the vertices, for nearest-vertex lookups.
struct Grid {
    cells: HashMap<[i32; 3], Vec<u32>>,
    points: Vec<Vec3>,
    cell: f32,
}

impl Grid {
    fn build(mesh: &Mesh) -> Self {
        let cell = (mesh.diagonal() / GRID_SPAN).max(1e-6);
        let points: Vec<Vec3> = mesh
            .positions
            .iter()
            .map(|p| Vec3::from_array(*p))
            .collect();
        let mut cells: HashMap<[i32; 3], Vec<u32>> = HashMap::new();
        for (i, p) in points.iter().enumerate() {
            cells.entry(cell_of(*p, cell)).or_default().push(i as u32);
        }
        Self {
            cells,
            points,
            cell,
        }
    }

    fn nearest(&self, p: Vec3) -> Option<f32> {
        let base = cell_of(p, self.cell);
        let mut best: Option<f32> = None;
        let mut radius = 0i32;
        loop {
            self.scan_shell(base, radius, p, &mut best);
            if let Some(distance) = best
                && distance <= radius as f32 * self.cell
            {
                return best;
            }
            radius += 1;
            if radius > MAX_RADIUS {
                return best;
            }
        }
    }

    fn scan_shell(&self, base: [i32; 3], radius: i32, p: Vec3, best: &mut Option<f32>) {
        for dx in -radius..=radius {
            for dy in -radius..=radius {
                for dz in -radius..=radius {
                    if dx.abs() != radius && dy.abs() != radius && dz.abs() != radius {
                        continue;
                    }
                    let key = [base[0] + dx, base[1] + dy, base[2] + dz];
                    let Some(indices) = self.cells.get(&key) else {
                        continue;
                    };
                    for &i in indices {
                        let distance = (self.points[i as usize] - p).length();
                        if best.is_none_or(|b| distance < b) {
                            *best = Some(distance);
                        }
                    }
                }
            }
        }
    }
}

fn cell_of(p: Vec3, cell: f32) -> [i32; 3] {
    [
        (p.x / cell).floor() as i32,
        (p.y / cell).floor() as i32,
        (p.z / cell).floor() as i32,
    ]
}
