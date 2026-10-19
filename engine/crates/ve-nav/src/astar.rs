//! Deterministic A* over the walkable grid.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::grid::Grid;

/// Movement cost of one cell step, in thousandths of a cell.
const CARDINAL: i64 = 1_000;
/// `sqrt(2) * 1000`, truncated: a diagonal step.
const DIAGONAL: i64 = 1_414;
/// Cost of climbing a metre, in the same units. Flat routes win ties.
const PER_METRE: i64 = 1_000;

/// Neighbour offsets `(dx, dz, cost)`, in a fixed order.
///
/// Diagonal steps are allowed only when both cardinal cells beside them are walkable (checked in
/// [`find_path_cells`]), so a diagonal never cuts a wall corner. The funnel receives a tiny,
/// non-zero portal at the shared corner for such steps; a zero-width portal would let a straight
/// line slip between the two boundaries.
const NEIGHBOURS: [(isize, isize, i64); 8] = [
    (1, 0, CARDINAL),
    (-1, 0, CARDINAL),
    (0, 1, CARDINAL),
    (0, -1, CARDINAL),
    (1, 1, DIAGONAL),
    (1, -1, DIAGONAL),
    (-1, 1, DIAGONAL),
    (-1, -1, DIAGONAL),
];

/// A* from `start` to `goal` cell indices.
///
/// Returns the cell sequence including both ends, or `None` when the goal is unreachable. The
/// search is deterministic: ties on `f` are broken by the lower cell index, and neighbours are
/// always visited in [`NEIGHBOURS`] order. A diagonal step is only allowed when both cardinal
/// cells beside it are walkable, so paths never cut through a wall corner.
pub(crate) fn find_path_cells(
    grid: &Grid,
    start: usize,
    goal: usize,
    max_step: f32,
) -> Option<Vec<u32>> {
    if start == goal {
        return Some(vec![start as u32]);
    }
    let cells = grid.nx() * grid.nz();
    let mut g = vec![i64::MAX; cells];
    let mut came = vec![u32::MAX; cells];
    let mut closed = vec![false; cells];
    let mut open = BinaryHeap::new();

    g[start] = 0;
    open.push(Reverse((heuristic(grid, start, goal), start as u32)));

    while let Some(Reverse((_, current))) = open.pop() {
        let current = current as usize;
        if closed[current] {
            continue;
        }
        if current == goal {
            return Some(reconstruct(&came, start, goal));
        }
        closed[current] = true;
        let (cx, cz) = grid.coords(current);
        for &(dx, dz, base) in &NEIGHBOURS {
            let nx = cx.cast_signed() + dx;
            let nz = cz.cast_signed() + dz;
            if nx < 0 || nz < 0 || nx as usize >= grid.nx() || nz as usize >= grid.nz() {
                continue;
            }
            let next = grid.index(nx as usize, nz as usize);
            if closed[next] || !grid.is_walkable(next) {
                continue;
            }
            let step = (grid.height(next) - grid.height(current)).abs();
            if step > max_step {
                continue;
            }
            if dx != 0 && dz != 0 {
                let side_a = grid.index((cx.cast_signed() + dx) as usize, cz);
                let side_b = grid.index(cx, (cz.cast_signed() + dz) as usize);
                if !grid.is_walkable(side_a) || !grid.is_walkable(side_b) {
                    continue;
                }
            }
            let cost = base + (step * PER_METRE as f32).round() as i64;
            let tentative = g[current].saturating_add(cost);
            if tentative < g[next] {
                g[next] = tentative;
                came[next] = current as u32;
                let f = tentative.saturating_add(heuristic(grid, next, goal));
                open.push(Reverse((f, next as u32)));
            }
        }
    }
    None
}

/// Octile distance in thousandths of a cell; admissible because it ignores height penalties.
fn heuristic(grid: &Grid, from: usize, to: usize) -> i64 {
    let (fx, fz) = grid.coords(from);
    let (tx, tz) = grid.coords(to);
    let dx = (fx as f32 - tx as f32).abs();
    let dz = (fz as f32 - tz as f32).abs();
    let (long, short) = if dx > dz { (dx, dz) } else { (dz, dx) };
    let cells = long + (std::f32::consts::SQRT_2 - 1.0) * short;
    (cells * 1000.0).round() as i64
}

fn reconstruct(came: &[u32], start: usize, goal: usize) -> Vec<u32> {
    let mut path = vec![goal as u32];
    let mut current = goal;
    while current != start {
        let parent = came[current] as usize;
        path.push(parent as u32);
        current = parent;
    }
    path.reverse();
    path
}
