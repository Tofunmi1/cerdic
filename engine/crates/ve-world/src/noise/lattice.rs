//! Value and gradient (Perlin) noise on the integer lattice, in two to four dimensions.

use super::hash::{cell_hash, gradient, unit};

/// Quintic fade: zero first and second derivatives at both ends, so cell borders show no creases.
fn quintic(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// The cell containing `p` along each axis, the position inside it, and the faded position.
fn locate(p: &[f64; 4], dims: usize) -> ([i64; 4], [f64; 4], [f64; 4]) {
    let (mut base, mut frac, mut fade) = ([0_i64; 4], [0.0; 4], [0.0; 4]);
    for axis in 0..dims {
        let floor = p[axis].floor();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "float-to-int saturates, which keeps absurd coordinates finite"
        )]
        {
            base[axis] = floor as i64;
        }
        frac[axis] = p[axis] - floor;
        fade[axis] = quintic(frac[axis]);
    }
    (base, frac, fade)
}

/// Visits the `2^dims` corners of the cell: its lattice coordinates, the offset from `p` to the corner, and the blend weight.
fn corners(p: &[f64; 4], dims: usize, mut visit: impl FnMut([i64; 4], [f64; 4], f64)) {
    let (base, frac, fade) = locate(p, dims);
    for corner in 0..(1_usize << dims) {
        let (mut cell, mut offset, mut weight) = (base, [0.0; 4], 1.0);
        for axis in 0..dims {
            let high = corner >> axis & 1 == 1;
            if high {
                cell[axis] = cell[axis].saturating_add(1);
                offset[axis] = frac[axis] - 1.0;
                weight *= fade[axis];
            } else {
                offset[axis] = frac[axis];
                weight *= 1.0 - fade[axis];
            }
        }
        visit(cell, offset, weight);
    }
}

/// Smoothly interpolated random corner values, in `[-1, 1]`.
pub(super) fn value(seed: u32, p: &[f64; 4], dims: usize) -> f64 {
    let mut sum = 0.0;
    corners(p, dims, |cell, _, weight| {
        sum += weight * (unit(cell_hash(seed, cell, dims)) * 2.0 - 1.0);
    });
    sum
}

/// Gradient noise. With unit gradients the largest possible value is `sqrt(dims) / 2`; the result is scaled to `[-1, 1]`.
pub(super) fn perlin(seed: u32, p: &[f64; 4], dims: usize) -> f64 {
    let mut sum = 0.0;
    corners(p, dims, |cell, offset, weight| {
        let g = gradient(cell_hash(seed, cell, dims), dims);
        let dot: f64 = (0..dims).map(|axis| g[axis] * offset[axis]).sum();
        sum += weight * dot;
    });
    #[expect(clippy::cast_precision_loss, reason = "dims is 2, 3 or 4")]
    let bound = (dims as f64).sqrt() / 2.0;
    sum / bound
}
