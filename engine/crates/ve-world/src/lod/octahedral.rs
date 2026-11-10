//! Octahedral direction packing and vertex-grid view blending, with +Y up.
//!
//! Hemisphere packing and triangle weights adapted from `RedGPU` (MIT),
//! `foliageOctahedralCommon.wgsl`, `bakeFoliageImpostor.ts`, and
//! `octahedralImpostorFragment.wgsl`.
//! Copyright (c) 2019~ Redcamel. Licensed under the MIT License.
//! Unlike the upstream bake, frames here sample grid vertices, not cell centres.

/// One atlas frame and its barycentric contribution to a view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewCell {
    /// Zero-based atlas column.
    pub col: u32,
    /// Zero-based atlas row.
    pub row: u32,
    /// Blend weight, between zero and one.
    pub weight: f64,
}

/// Packs a full-sphere direction into the unit square.
///
/// +Y maps to the centre; -Y maps to a corner. Signed zeros preserve which
/// copy of a seam direction was produced by [`decode`]. Invalid directions
/// (zero or any non-finite component) behave as +Y.
#[must_use]
pub fn encode(dir: [f64; 3]) -> [f64; 2] {
    let [x, y, z] = scaled_direction(dir);
    let length = x.abs() + y.abs() + z.abs();
    let (mut packed_x, mut packed_z) = (x / length, z / length);
    if y < 0.0 {
        (packed_x, packed_z) = (
            (1.0 - packed_z.abs()).copysign(x),
            (1.0 - packed_x.abs()).copysign(z),
        );
    }
    [packed_x * 0.5 + 0.5, packed_z * 0.5 + 0.5]
}

/// Unpacks a full-sphere direction; coordinates are clamped to the unit square.
/// Non-finite coordinates select the centre. The result is a unit vector.
#[must_use]
pub fn decode(uv: [f64; 2]) -> [f64; 3] {
    let [u, v] = square_coordinates(uv);
    let height = 1.0 - u.abs() - v.abs();
    let (x, z) = if height < 0.0 {
        ((1.0 - v.abs()).copysign(u), (1.0 - u.abs()).copysign(v))
    } else {
        (u, v)
    };
    unit([x, height, z])
}

/// Packs the upper hemisphere into the whole unit square, with +Y at its centre.
/// Directions below the horizon use their horizontal projection; straight down
/// and invalid directions fall back to +Y.
#[must_use]
pub fn hemi_encode(dir: [f64; 3]) -> [f64; 2] {
    let [x, y, z] = scaled_direction(dir);
    let length = x.abs() + y.max(0.0) + z.abs();
    if length <= 0.0 {
        return [0.5; 2];
    }
    [(x + z) / length * 0.5 + 0.5, (z - x) / length * 0.5 + 0.5]
}

/// Unpacks a unit upper-hemisphere direction (the square boundary is the horizon).
/// Coordinates are clamped; non-finite coordinates select +Y.
#[must_use]
pub fn hemi_decode(uv: [f64; 2]) -> [f64; 3] {
    let [u, v] = square_coordinates(uv);
    let (x, z) = ((u - v) * 0.5, (u + v) * 0.5);
    unit([x, (1.0 - x.abs() - z.abs()).max(0.0), z])
}

/// Direction from the model centre towards a frame's bake camera.
///
/// Frames sample `(col / (grid - 1), row / (grid - 1))`. Out-of-range indices
/// clamp to the grid; grids smaller than two describe a single +Y frame.
#[must_use]
pub fn frame_direction(col: u32, row: u32, grid: u32, hemi: bool) -> [f64; 3] {
    if grid < 2 {
        return [0.0, 1.0, 0.0];
    }
    let last = grid - 1;
    let uv = [
        f64::from(col.min(last)) / f64::from(last),
        f64::from(row.min(last)) / f64::from(last),
    ];
    if hemi { hemi_decode(uv) } else { decode(uv) }
}

/// Selects the three vertices of the containing grid triangle and their weights.
///
/// Each cell is split along its top-right to bottom-left diagonal. Boundary
/// samples stay in the last cell. Invalid directions behave as +Y. For grids
/// smaller than two, frame (0, 0) receives all the weight.
#[must_use]
pub fn select_views(dir: [f64; 3], grid: u32, hemi: bool) -> [ViewCell; 3] {
    if grid < 2 {
        return [
            ViewCell {
                col: 0,
                row: 0,
                weight: 1.0,
            },
            ViewCell {
                col: 0,
                row: 0,
                weight: 0.0,
            },
            ViewCell {
                col: 0,
                row: 0,
                weight: 0.0,
            },
        ];
    }
    let uv = if hemi { hemi_encode(dir) } else { encode(dir) };
    let (col, x) = grid_coordinate(uv[0], grid);
    let (row, y) = grid_coordinate(uv[1], grid);
    if x + y <= 1.0 {
        [
            ViewCell {
                col,
                row,
                weight: 1.0 - (x + y),
            },
            ViewCell {
                col: col + 1,
                row,
                weight: x,
            },
            ViewCell {
                col,
                row: row + 1,
                weight: y,
            },
        ]
    } else {
        [
            ViewCell {
                col: col + 1,
                row: row + 1,
                weight: x + y - 1.0,
            },
            ViewCell {
                col: col + 1,
                row,
                weight: 1.0 - y,
            },
            ViewCell {
                col,
                row: row + 1,
                weight: 1.0 - x,
            },
        ]
    }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "grid coordinates are finite, nonnegative and bounded by a u32 grid"
)]
fn grid_coordinate(uv: f64, grid: u32) -> (u32, f64) {
    let position = uv * f64::from(grid - 1);
    let base = (position.floor() as u32).min(grid - 2);
    (base, position - f64::from(base))
}

fn square_coordinates(uv: [f64; 2]) -> [f64; 2] {
    if uv.iter().any(|value| !value.is_finite()) {
        [0.0; 2]
    } else {
        uv.map(|value| value.clamp(0.0, 1.0) * 2.0 - 1.0)
    }
}

// Scaling before summing avoids overflow and underflow for finite directions.
fn scaled_direction(dir: [f64; 3]) -> [f64; 3] {
    let scale = dir.iter().map(|value| value.abs()).fold(0.0_f64, f64::max);
    if dir.iter().any(|value| !value.is_finite()) || scale <= 0.0 {
        [0.0, 1.0, 0.0]
    } else {
        dir.map(|value| value / scale)
    }
}

fn unit(dir: [f64; 3]) -> [f64; 3] {
    let length = dir[0].hypot(dir[1]).hypot(dir[2]);
    dir.map(|value| value / length)
}
