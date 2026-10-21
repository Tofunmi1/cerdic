//! Flow routing and river masks.
use std::cmp::Ordering;
use std::collections::{BinaryHeap, VecDeque};

use super::{ErosionError, Grid};
use crate::num::{cell_u32, offset};

/// Where water goes from each cell.
#[derive(Debug, Clone, PartialEq)]
pub struct Flow {
    /// Steepest-descent D8 neighbour after pit filling; `u32::MAX` at sinks and outlets.
    pub receiver: Vec<u32>,
    /// Square metres draining through each cell, including its own.
    pub area: Vec<f32>,
}

#[derive(Clone, Copy)]
struct Entry {
    height: f32,
    index: usize,
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.height == other.height && self.index == other.index
    }
}
impl Eq for Entry {}
impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .height
            .total_cmp(&self.height)
            .then_with(|| other.index.cmp(&self.index))
    }
}

/// Routes water over the grid. Pits are filled on a copy; the grid is not changed.
///
/// # Errors
/// `Cancelled`, `BadShape`, or `TooLarge`.
pub fn route_flow(grid: &Grid<'_>, cancel: &dyn Fn() -> bool) -> Result<Flow, ErosionError> {
    grid.check()?;
    let count = grid.width * grid.depth;
    let (filled, parent) = fill_pits(grid, count, cancel)?;
    let receiver = choose_receivers(grid, &filled, &parent, count, cancel)?;
    let area = accumulate_area(grid, &receiver, count, cancel)?;
    Ok(Flow { receiver, area })
}

fn fill_pits(
    grid: &Grid<'_>,
    count: usize,
    cancel: &dyn Fn() -> bool,
) -> Result<(Vec<f32>, Vec<u32>), ErosionError> {
    let mut filled = grid.heights.to_vec();
    let mut visited = vec![false; count];
    let mut heap = BinaryHeap::new();
    add_outlets(grid, &mut visited, &mut heap, cancel)?;
    let mut parent = vec![u32::MAX; count];
    while let Some(entry) = heap.pop() {
        visit_neighbors(
            grid,
            entry,
            &mut visited,
            &mut filled,
            &mut parent,
            &mut heap,
        );
    }
    Ok((filled, parent))
}

fn add_outlets(
    grid: &Grid<'_>,
    visited: &mut [bool],
    heap: &mut BinaryHeap<Entry>,
    cancel: &dyn Fn() -> bool,
) -> Result<(), ErosionError> {
    for z in 0..grid.depth {
        if cancel() {
            return Err(ErosionError::Cancelled);
        }
        for x in 0..grid.width {
            if x == 0 || z == 0 || x + 1 == grid.width || z + 1 == grid.depth {
                let index = z * grid.width + x;
                visited[index] = true;
                heap.push(Entry {
                    height: grid.heights[index],
                    index,
                });
            }
        }
    }
    Ok(())
}

fn visit_neighbors(
    grid: &Grid<'_>,
    entry: Entry,
    visited: &mut [bool],
    filled: &mut [f32],
    parent: &mut [u32],
    heap: &mut BinaryHeap<Entry>,
) {
    let x = entry.index % grid.width;
    let z = entry.index / grid.width;
    for (_, _, neighbor) in neighbor_cells(x, z, grid.width, grid.depth) {
        let Some(index) = neighbor else {
            continue;
        };
        if visited[index] {
            continue;
        }
        visited[index] = true;
        parent[index] = cell_u32(entry.index);
        filled[index] = filled[index].max(entry.height);
        heap.push(Entry {
            height: filled[index],
            index,
        });
    }
}

fn choose_receivers(
    grid: &Grid<'_>,
    filled: &[f32],
    parent: &[u32],
    count: usize,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<u32>, ErosionError> {
    let mut receiver = vec![u32::MAX; count];
    for z in 0..grid.depth {
        if cancel() {
            return Err(ErosionError::Cancelled);
        }
        for x in 0..grid.width {
            let index = z * grid.width + x;
            receiver[index] = steepest_neighbor(grid, filled, parent, x, z, index);
        }
    }
    Ok(receiver)
}

fn steepest_neighbor(
    grid: &Grid<'_>,
    filled: &[f32],
    parent: &[u32],
    x: usize,
    z: usize,
    index: usize,
) -> u32 {
    let mut best = f32::NEG_INFINITY;
    let mut receiver = u32::MAX;
    for (delta_x, delta_z, neighbor) in neighbor_cells(x, z, grid.width, grid.depth) {
        let Some(neighbor) = neighbor else {
            continue;
        };
        let diagonal = delta_x != 0 && delta_z != 0;
        let distance = grid.cell
            * if diagonal {
                std::f32::consts::SQRT_2
            } else {
                1.0
            };
        let slope = (filled[index] - filled[neighbor]) / distance;
        if slope > best {
            best = slope;
            receiver = cell_u32(neighbor);
        }
    }
    if best <= 0.0 { parent[index] } else { receiver }
}

fn accumulate_area(
    grid: &Grid<'_>,
    receiver: &[u32],
    count: usize,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<f32>, ErosionError> {
    let mut area = vec![grid.cell * grid.cell; count];
    let mut downstream_count = vec![0_usize; count];
    for &downstream in receiver {
        if downstream != u32::MAX
            && let Ok(index) = usize::try_from(downstream)
        {
            downstream_count[index] += 1;
        }
    }
    let mut ready: VecDeque<usize> = downstream_count
        .iter()
        .enumerate()
        .filter_map(|(index, count)| if *count == 0 { Some(index) } else { None })
        .collect();
    while let Some(index) = ready.pop_front() {
        if cancel() {
            return Err(ErosionError::Cancelled);
        }
        let downstream = receiver[index];
        if downstream != u32::MAX
            && let Ok(downstream) = usize::try_from(downstream)
        {
            area[downstream] += area[index];
            downstream_count[downstream] -= 1;
            if downstream_count[downstream] == 0 {
                ready.push_back(downstream);
            }
        }
    }
    Ok(area)
}

fn neighbor_cells(
    x: usize,
    z: usize,
    width: usize,
    depth: usize,
) -> impl Iterator<Item = (i32, i32, Option<usize>)> {
    const DIRECTIONS: [(i32, i32); 8] = [
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ];
    DIRECTIONS.into_iter().map(move |(delta_x, delta_z)| {
        let neighbor_x = offset(x, delta_x, width);
        let neighbor_z = offset(z, delta_z, depth);
        let neighbor = neighbor_x.zip(neighbor_z).map(|(x, z)| z * width + x);
        (delta_x, delta_z, neighbor)
    })
}

/// Returns a 0..=1 river mask ramping from half to twice `min_area`.
#[must_use]
pub fn river_mask(flow: &Flow, width: usize, depth: usize, min_area: f32) -> Vec<f32> {
    if width.checked_mul(depth) != Some(flow.area.len()) || min_area <= 0.0 {
        return vec![0.0; width.saturating_mul(depth)];
    }
    let low = min_area * 0.5;
    let high = min_area * 2.0;
    flow.area
        .iter()
        .map(|area| {
            let t = ((*area - low) / (high - low)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        })
        .collect()
}
