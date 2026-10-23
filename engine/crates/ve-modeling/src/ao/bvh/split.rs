//! Where a box of triangles is cut in two: the surface area heuristic over a few bins, so big triangles and small ones end up in boxes of
//! their own, with a median split when the heuristic has nothing to say. The cut changes how fast a ray finds a hit, never whether it does.
use super::Triangle;

const BINS: usize = 12;
const BINS_F: f64 = 12.0;

/// A triangle count as a weight for the area heuristic.
fn weight(count: usize) -> f64 {
    f64::from(u32::try_from(count).unwrap_or(u32::MAX))
}
/// Deeper than this the cut is always the median, which keeps the tree shallow enough for the traversal stack.
pub(super) const MAX_SAH_DEPTH: usize = 28;

#[derive(Clone, Copy)]
struct Bin {
    min: [f64; 3],
    max: [f64; 3],
    count: usize,
}

const EMPTY: Bin = Bin {
    min: [f64::INFINITY; 3],
    max: [f64::NEG_INFINITY; 3],
    count: 0,
};

fn grow(bin: &mut Bin, min: [f64; 3], max: [f64; 3]) {
    for axis in 0..3 {
        bin.min[axis] = bin.min[axis].min(min[axis]);
        bin.max[axis] = bin.max[axis].max(max[axis]);
    }
}

fn area(min: [f64; 3], max: [f64; 3]) -> f64 {
    let d = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
    2.0 * (d[0] * d[1] + d[1] * d[2] + d[2] * d[0])
}

/// Reorders `order` so the first part goes in the left child, and returns how many triangles that is (never 0 or all of them).
pub(super) fn partition(order: &mut [usize], triangles: &[Triangle], depth: usize) -> usize {
    if depth < MAX_SAH_DEPTH
        && let Some(count) = surface_area_cut(order, triangles)
    {
        return count;
    }
    median(order, triangles)
}

fn median(order: &mut [usize], triangles: &[Triangle]) -> usize {
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for id in order.iter() {
        for axis in 0..3 {
            lo[axis] = lo[axis].min(triangles[*id].center[axis]);
            hi[axis] = hi[axis].max(triangles[*id].center[axis]);
        }
    }
    let spread = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
    let axis = if spread[0] >= spread[1] && spread[0] >= spread[2] {
        0
    } else if spread[1] >= spread[2] {
        1
    } else {
        2
    };
    let left = order.len() / 2;
    // Only the median has to be in place. Ties go by index, so the cut is the same on every run.
    order.select_nth_unstable_by(left, |a, b| {
        triangles[*a].center[axis]
            .total_cmp(&triangles[*b].center[axis])
            .then_with(|| a.cmp(b))
    });
    left
}

fn surface_area_cut(order: &mut [usize], triangles: &[Triangle]) -> Option<usize> {
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for id in order.iter() {
        for axis in 0..3 {
            lo[axis] = lo[axis].min(triangles[*id].center[axis]);
            hi[axis] = hi[axis].max(triangles[*id].center[axis]);
        }
    }
    let mut best: Option<(f64, usize, usize)> = None;
    for axis in 0..3 {
        let spread = hi[axis] - lo[axis];
        if spread <= 1.0e-12 {
            continue;
        }
        let mut bins = [EMPTY; BINS];
        for id in order.iter() {
            let t = &triangles[*id];
            let bin = &mut bins[bin_of(t.center[axis], lo[axis], spread)];
            bin.count += 1;
            grow(bin, t.min, t.max);
        }
        // Cost of cutting after bin `cut - 1`: each side's area times its triangle count.
        let mut left_cost = [0.0_f64; BINS];
        let mut running = EMPTY;
        for cut in 1..BINS {
            let bin = bins[cut - 1];
            running.count += bin.count;
            if bin.count > 0 {
                grow(&mut running, bin.min, bin.max);
            }
            left_cost[cut] = if running.count > 0 {
                area(running.min, running.max) * weight(running.count)
            } else {
                0.0
            };
        }
        let mut running = EMPTY;
        for cut in (1..BINS).rev() {
            let bin = bins[cut];
            running.count += bin.count;
            if bin.count > 0 {
                grow(&mut running, bin.min, bin.max);
            }
            let left_count: usize = bins[..cut].iter().map(|b| b.count).sum();
            if running.count == 0 || left_count == 0 {
                continue;
            }
            let cost = left_cost[cut] + area(running.min, running.max) * weight(running.count);
            if best.is_none_or(|(c, _, _)| cost < c) {
                best = Some((cost, axis, cut));
            }
        }
    }
    let (_, axis, cut) = best?;
    let spread = hi[axis] - lo[axis];
    // Partition in place: a triangle goes left when its centre falls in a bin before the cut.
    let mut left = 0;
    for i in 0..order.len() {
        if bin_of(triangles[order[i]].center[axis], lo[axis], spread) < cut {
            order.swap(i, left);
            left += 1;
        }
    }
    (left > 0 && left < order.len()).then_some(left)
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the position is clamped to the bin range"
)]
fn bin_of(center: f64, lo: f64, spread: f64) -> usize {
    (((center - lo) / spread * BINS_F) as usize).min(BINS - 1)
}
