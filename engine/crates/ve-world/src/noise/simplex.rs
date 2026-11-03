//! Simplex noise in two to four dimensions (the skewed-lattice construction: each point sums `dims + 1` kernels, not `2^dims`).
//! Following the structure of `FastNoise2`'s simplex generators (MIT, Jordan Peck), written generically over the dimension.

use super::hash::{cell_hash, gradient};

/// Scales the kernel sum to about `[-1, 1]` for unit gradients; the caller clamps what is left over.
const SCALE: [f64; 5] = [0.0, 0.0, 99.0, 45.3, 46.8];
/// Kernel radius squared: the kernel must reach zero before the neighbouring simplex takes over.
const RADIUS: [f64; 5] = [0.0, 0.0, 0.5, 0.6, 0.6];

pub(super) fn simplex(seed: u32, p: &[f64; 4], dims: usize) -> f64 {
    #[expect(clippy::cast_precision_loss, reason = "dims is 2, 3 or 4")]
    let n = dims as f64;
    let root = (n + 1.0).sqrt();
    let skew = (root - 1.0) / n;
    let unskew = (1.0 - 1.0 / root) / n;

    let shift: f64 = p[..dims].iter().sum::<f64>() * skew;
    let (mut cell, mut floors) = ([0_i64; 4], [0.0; 4]);
    for axis in 0..dims {
        floors[axis] = (p[axis] + shift).floor();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "float-to-int saturates, which keeps absurd coordinates finite"
        )]
        {
            cell[axis] = floors[axis] as i64;
        }
    }
    let back: f64 = floors[..dims].iter().sum::<f64>() * unskew;
    let mut from_origin = [0.0; 4];
    for axis in 0..dims {
        from_origin[axis] = p[axis] - (floors[axis] - back);
    }

    // The simplex containing the point is walked along the axes in descending order of the offset (ties by axis, so it is deterministic).
    let mut order = [0_usize, 1, 2, 3];
    for i in 1..dims {
        let mut j = i;
        while j > 0 && from_origin[order[j]] > from_origin[order[j - 1]] {
            order.swap(j, j - 1);
            j -= 1;
        }
    }

    let mut sum = 0.0;
    let mut step = [0_i64; 4];
    for corner in 0..=dims {
        if corner > 0 {
            step[order[corner - 1]] += 1;
        }
        #[expect(clippy::cast_precision_loss, reason = "corner is at most 4")]
        let pull = corner as f64 * unskew;
        let (mut position, mut distance) = ([0.0; 4], 0.0);
        for axis in 0..dims {
            #[expect(clippy::cast_precision_loss, reason = "step is 0 or 1")]
            let moved = step[axis] as f64;
            position[axis] = from_origin[axis] - moved + pull;
            distance += position[axis] * position[axis];
        }
        let falloff = RADIUS[dims] - distance;
        if falloff > 0.0 {
            let squared = falloff * falloff;
            let mut at = cell;
            for axis in 0..dims {
                at[axis] = at[axis].saturating_add(step[axis]);
            }
            let g = gradient(cell_hash(seed, at, dims), dims);
            let dot: f64 = (0..dims).map(|axis| g[axis] * position[axis]).sum();
            sum += squared * squared * dot;
        }
    }
    sum * SCALE[dims]
}
