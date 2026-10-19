//! Line of sight across the grid.

use glam::Vec2;

use crate::grid::Grid;

/// Marches the segment `a -> b` across cells and returns the first point where it enters a blocked
/// cell, or `None` when the whole line stays on walkable ground.
///
/// A point outside the grid counts as blocked at that end: sight is defined inside the level. The
/// traversal is a grid DDA, so it visits each crossed cell exactly once.
pub(crate) fn raycast(grid: &Grid, a: [f32; 3], b: [f32; 3]) -> Option<[f32; 3]> {
    let a2 = Vec2::new(a[0], a[2]);
    let b2 = Vec2::new(b[0], b[2]);
    let delta = b2 - a2;
    let length = delta.length();
    if length <= 1.0e-9 {
        return (!grid.is_walkable_at(a2)).then_some(a);
    }
    let Some((ix, iz)) = grid.cell_at(a2) else {
        return Some(a);
    };
    if !grid.is_walkable(grid.index(ix, iz)) {
        return Some(a);
    }
    march(grid, a, b, a2, (ix, iz), delta / length, length)
}

/// The DDA itself: walks cell borders from the start cell to the end of the segment.
fn march(
    grid: &Grid,
    a: [f32; 3],
    b: [f32; 3],
    a2: Vec2,
    start: (usize, usize),
    dir: Vec2,
    length: f32,
) -> Option<[f32; 3]> {
    let (mut ix, mut iz) = start;
    let cell = grid.cell();
    let [ox, oz] = grid.origin();
    let step_x = if dir.x > 0.0 {
        1
    } else if dir.x < 0.0 {
        -1
    } else {
        0
    };
    let step_z = if dir.y > 0.0 {
        1
    } else if dir.y < 0.0 {
        -1
    } else {
        0
    };
    let t_delta_x = if step_x == 0 {
        f32::INFINITY
    } else {
        cell / dir.x.abs()
    };
    let t_delta_z = if step_z == 0 {
        f32::INFINITY
    } else {
        cell / dir.y.abs()
    };
    let next_x = if step_x > 0 {
        ox + (ix as f32 + 1.0) * cell
    } else {
        ox + ix as f32 * cell
    };
    let next_z = if step_z > 0 {
        oz + (iz as f32 + 1.0) * cell
    } else {
        oz + iz as f32 * cell
    };
    let mut t_max_x = if step_x == 0 {
        f32::INFINITY
    } else {
        (next_x - a2.x) / dir.x
    };
    let mut t_max_z = if step_z == 0 {
        f32::INFINITY
    } else {
        (next_z - a2.y) / dir.y
    };

    loop {
        let t;
        if t_max_x < t_max_z {
            let next = ix.cast_signed() + step_x as isize;
            if next < 0 || next as usize >= grid.nx() {
                return None;
            }
            ix = next as usize;
            t = t_max_x;
            t_max_x += t_delta_x;
        } else {
            let next = iz.cast_signed() + step_z as isize;
            if next < 0 || next as usize >= grid.nz() {
                return None;
            }
            iz = next as usize;
            t = t_max_z;
            t_max_z += t_delta_z;
        }
        if t > length {
            return None;
        }
        if !grid.is_walkable(grid.index(ix, iz)) {
            let f = (t / length).clamp(0.0, 1.0);
            return Some([
                a[0] + (b[0] - a[0]) * f,
                a[1] + (b[1] - a[1]) * f,
                a[2] + (b[2] - a[2]) * f,
            ]);
        }
    }
}
