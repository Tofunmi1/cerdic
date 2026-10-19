//! Spatial grid pair preflight and deterministic union-find.

use std::collections::BTreeMap;

use ve_kernel::ExecutionContext;

use crate::topology::check_cancelled;
use crate::{GeometryError, PolyMesh};

pub(super) type Cell = (i64, i64, i64);
const CELL_COORDINATE_LIMIT: f64 = 4_503_599_627_370_496.0;

pub(super) fn build_cells(
    mesh: &PolyMesh,
    tolerance: f64,
    context: &ExecutionContext,
) -> Result<BTreeMap<Cell, Vec<usize>>, GeometryError> {
    let mut cells = BTreeMap::<Cell, Vec<usize>>::new();
    for (index, point) in mesh.positions().iter().enumerate() {
        if index % 1024 == 0 {
            check_cancelled(context)?;
        }
        let cell = (
            cell_coordinate(point[0], tolerance)?,
            cell_coordinate(point[1], tolerance)?,
            cell_coordinate(point[2], tolerance)?,
        );
        cells.entry(cell).or_default().push(index);
    }
    Ok(cells)
}

fn cell_coordinate(value: f32, tolerance: f64) -> Result<i64, GeometryError> {
    let scaled = (f64::from(value) / tolerance).floor();
    if !scaled.is_finite() || !(-CELL_COORDINATE_LIMIT..=CELL_COORDINATE_LIMIT).contains(&scaled) {
        return Err(GeometryError::PolyRefusal {
            code: "poly.weld_grid_overflow",
            element: None,
        });
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the floored coordinate is checked to fit the i64 range"
    )]
    Ok(scaled as i64)
}

fn neighbor_cells(cell: Cell) -> Vec<Cell> {
    let mut neighbors = Vec::with_capacity(27);
    for dx in -1_i64..=1 {
        for dy in -1_i64..=1 {
            for dz in -1_i64..=1 {
                if let (Some(x), Some(y), Some(z)) = (
                    cell.0.checked_add(dx),
                    cell.1.checked_add(dy),
                    cell.2.checked_add(dz),
                ) {
                    neighbors.push((x, y, z));
                }
            }
        }
    }
    neighbors.sort_unstable();
    neighbors
}

pub(super) fn count_candidate_pairs(
    cells: &BTreeMap<Cell, Vec<usize>>,
    context: &ExecutionContext,
) -> Result<usize, GeometryError> {
    let mut count = 0_usize;
    for (index, (cell, indices)) in cells.iter().enumerate() {
        if index % 256 == 0 {
            check_cancelled(context)?;
        }
        count = count
            .checked_add(pair_count(indices.len())?)
            .ok_or(pair_budget_error())?;
        for neighbor in neighbor_cells(*cell)
            .into_iter()
            .filter(|neighbor| neighbor > cell)
        {
            if let Some(adjacent) = cells.get(&neighbor) {
                count = count
                    .checked_add(
                        indices
                            .len()
                            .checked_mul(adjacent.len())
                            .ok_or(pair_budget_error())?,
                    )
                    .ok_or(pair_budget_error())?;
            }
        }
    }
    Ok(count)
}

fn pair_count(count: usize) -> Result<usize, GeometryError> {
    let (left, right) = if count.is_multiple_of(2) {
        (count / 2, count.saturating_sub(1))
    } else {
        (count, count.saturating_sub(1) / 2)
    };
    left.checked_mul(right).ok_or(pair_budget_error())
}

fn pair_budget_error() -> GeometryError {
    GeometryError::PolyRefusal {
        code: "poly.weld_pair_budget",
        element: None,
    }
}

pub(super) fn compare_candidate_pairs(
    mesh: &PolyMesh,
    tolerance: f64,
    cells: &BTreeMap<Cell, Vec<usize>>,
    parents: &mut [usize],
    context: &ExecutionContext,
) -> Result<(), GeometryError> {
    let tolerance_squared = tolerance * tolerance;
    let mut tested = 0_usize;
    for (cell, indices) in cells {
        for &vertex in indices {
            if vertex % 1024 == 0 {
                check_cancelled(context)?;
            }
            for neighbor in neighbor_cells(*cell) {
                let Some(candidates) = cells.get(&neighbor) else {
                    continue;
                };
                for &other in candidates.iter().filter(|&&other| other > vertex) {
                    tested += 1;
                    if tested.is_multiple_of(1024) {
                        check_cancelled(context)?;
                    }
                    if distance_squared(mesh.positions()[vertex], mesh.positions()[other])
                        <= tolerance_squared
                    {
                        union_minimum(parents, vertex, other);
                    }
                }
            }
        }
    }
    Ok(())
}

fn distance_squared(a: [f32; 3], b: [f32; 3]) -> f64 {
    (0..3)
        .map(|axis| {
            let delta = f64::from(a[axis]) - f64::from(b[axis]);
            delta * delta
        })
        .sum()
}

fn find_root(parents: &mut [usize], vertex: usize) -> usize {
    let mut root = vertex;
    while parents[root] != root {
        root = parents[root];
    }
    let mut current = vertex;
    while parents[current] != current {
        let next = parents[current];
        parents[current] = root;
        current = next;
    }
    root
}

fn union_minimum(parents: &mut [usize], a: usize, b: usize) {
    let root_a = find_root(parents, a);
    let root_b = find_root(parents, b);
    if root_a != root_b {
        let (minimum, maximum) = if root_a < root_b {
            (root_a, root_b)
        } else {
            (root_b, root_a)
        };
        parents[maximum] = minimum;
    }
}

pub(super) fn representatives(
    parents: &mut [usize],
    context: &ExecutionContext,
) -> Result<(Vec<usize>, Vec<u32>), GeometryError> {
    let mut roots = Vec::with_capacity(parents.len());
    for vertex in 0..parents.len() {
        if vertex % 1024 == 0 {
            check_cancelled(context)?;
        }
        roots.push(find_root(parents, vertex));
    }
    let mut root_to_new = BTreeMap::new();
    let mut representatives = Vec::new();
    let mut remap = Vec::with_capacity(roots.len());
    for (index, root) in roots.into_iter().enumerate() {
        if index % 1024 == 0 {
            check_cancelled(context)?;
        }
        let next = if let Some(&index) = root_to_new.get(&root) {
            index
        } else {
            let index = u32::try_from(representatives.len())
                .map_err(|_| GeometryError::InvalidMesh("weld output exceeds u32 vertex ids"))?;
            root_to_new.insert(root, index);
            representatives.push(root);
            index
        };
        remap.push(next);
    }
    Ok((representatives, remap))
}
