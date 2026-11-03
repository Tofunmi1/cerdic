//! Cellular (Worley) noise: one jittered feature point per lattice cell; the result comes from the nearest and second-nearest points.

use super::hash::{cell_hash, mix, unit};
use super::{CellDistance, CellReturn};

fn metric(distance: CellDistance, delta: &[f64; 4], dims: usize) -> f64 {
    let squared: f64 = delta[..dims].iter().map(|d| d * d).sum();
    let manhattan: f64 = delta[..dims].iter().map(|d| d.abs()).sum();
    match distance {
        CellDistance::Euclidean => squared.sqrt(),
        CellDistance::Manhattan => manhattan,
        CellDistance::Hybrid => squared + manhattan,
    }
}

/// Roughly the largest nearest-point distance for each metric, used to map distances onto `[-1, 1]`.
fn divisor(distance: CellDistance) -> f64 {
    match distance {
        CellDistance::Euclidean => 1.0,
        CellDistance::Manhattan => 1.4,
        CellDistance::Hybrid => 2.0,
    }
}

pub(super) fn cellular(
    seed: u32,
    p: &[f64; 4],
    dims: usize,
    distance: CellDistance,
    ret: CellReturn,
    jitter: f64,
) -> f64 {
    let mut base = [0_i64; 4];
    for axis in 0..dims {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "float-to-int saturates, which keeps absurd coordinates finite"
        )]
        {
            base[axis] = p[axis].floor() as i64;
        }
    }
    let (mut nearest, mut second, mut value) = (f64::MAX, f64::MAX, 0.0);
    // The nearest feature point is always in the 3^dims cells around the point's own cell.
    for index in 0..3_usize.pow(u32::try_from(dims).unwrap_or(4)) {
        let (mut cell, mut remaining) = (base, index);
        for slot in cell.iter_mut().take(dims) {
            *slot = slot.saturating_add(i64::try_from(remaining % 3).unwrap_or(1) - 1);
            remaining /= 3;
        }
        let h = cell_hash(seed, cell, dims);
        let mut delta = [0.0; 4];
        for axis in 0..dims {
            #[expect(clippy::cast_precision_loss, reason = "lattice offsets here are small")]
            let origin = cell[axis] as f64;
            let spread = unit(mix(h ^ (axis as u64 + 1))) - 0.5;
            delta[axis] = origin + 0.5 + jitter * spread - p[axis];
        }
        let d = metric(distance, &delta, dims);
        if d < nearest {
            second = nearest;
            nearest = d;
            value = unit(mix(h ^ 0xC2B2_AE3D_27D4_EB4F)) * 2.0 - 1.0;
        } else if d < second {
            second = d;
        }
    }
    let scale = divisor(distance);
    let mapped = match ret {
        CellReturn::Value => value,
        CellReturn::Distance => nearest / scale * 1.2 - 1.0,
        CellReturn::Distance2 => second / scale * 1.2 - 1.0,
        CellReturn::Difference => (second - nearest) / scale * 1.5 - 1.0,
    };
    mapped.clamp(-1.0, 1.0)
}
