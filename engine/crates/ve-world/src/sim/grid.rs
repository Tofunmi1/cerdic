//! A uniform grid over a vec3 array, so a neighbour query looks at nearby cells instead of every element.
//!
//! The grid only prunes: callers test the exact distance on what it returns, and candidates come back in ascending element order, so the
//! result of a `neighbors` loop is the same as testing every element in order.
use std::collections::HashMap;

/// The most cells away from the query's own cell the grid will search; a bigger radius falls back to every element.
const MAX_REACH: i32 = 3;

pub(super) struct Grid {
    pub(super) arr: usize,
    cell: f32,
    cells: HashMap<(i32, i32, i32), Vec<u32>>,
}

#[expect(clippy::cast_possible_truncation, reason = "the coordinate is clamped to a range an i32 holds")]
fn key(p: [f32; 3], cell: f32) -> (i32, i32, i32) {
    let k = |x: f32| (x / cell).floor().clamp(-1.0e9, 1.0e9) as i32;
    (k(p[0]), k(p[1]), k(p[2]))
}

impl Grid {
    /// `None` when `cell` cannot size a grid (not finite or not positive).
    pub(super) fn build(arr: usize, len: usize, cell: f32, at: impl Fn(usize) -> [f32; 3]) -> Option<Self> {
        if !cell.is_finite() || cell <= 0.0 {
            return None;
        }
        let mut cells: HashMap<(i32, i32, i32), Vec<u32>> = HashMap::new();
        for j in 0..len {
            let p = at(j);
            if p.iter().all(|x| x.is_finite()) {
                cells.entry(key(p, cell)).or_default().push(u32::try_from(j).unwrap_or(u32::MAX));
            }
        }
        Some(Self { arr, cell, cells })
    }

    /// Element numbers that may lie within `radius` of `centre`, ascending, or `None` when the radius is too big for the grid.
    pub(super) fn candidates(&self, centre: [f32; 3], radius: f32) -> Option<Vec<usize>> {
        if !radius.is_finite() || radius < 0.0 {
            return Some(Vec::new());
        }
        #[expect(clippy::cast_possible_truncation, reason = "the reach is small and checked right below")]
        let reach = (radius / self.cell).ceil().min(1.0e6) as i32;
        if reach > MAX_REACH {
            return None;
        }
        let (cx, cy, cz) = key(centre, self.cell);
        let mut out = Vec::new();
        for x in cx - reach..=cx + reach {
            for y in cy - reach..=cy + reach {
                for z in cz - reach..=cz + reach {
                    if let Some(list) = self.cells.get(&(x, y, z)) {
                        out.extend(list.iter().map(|&j| j as usize));
                    }
                }
            }
        }
        out.sort_unstable();
        Some(out)
    }
}
