//! A sufficient test for a swept tube that runs through itself.
//!
//! A straight path segment sweeps its profile into a cylinder that contains the cylinder of the
//! profile's inscribed radius around the origin. Two non-adjacent segments whose closest points are
//! both interior to their segments, and closer than twice that radius, therefore have overlapping
//! cylinders: the sweep is a self-intersecting solid. The test never refuses a valid tube. It does
//! not find every overlap: an end cap poking into another part of the tube (closest point at a path
//! end or a bend vertex) and exactly parallel, overlapping segments are not detected.

use super::vec3::{dot, length, sub};
use crate::Fail;
use ve_kernel::ExecutionContext;

/// Two path segments whose tubes overlap.
pub(super) struct Overlap {
    pub(super) first: usize,
    pub(super) second: usize,
    pub(super) distance: f64,
    pub(super) needed: f64,
}

/// The inscribed radius of a strictly convex profile around the origin, or `None` when the origin is
/// not strictly inside it (no claim can then be made about overlap).
fn inscribed_radius(profile: &[[f64; 2]]) -> Option<f64> {
    let mut sign = 0.0_f64;
    let mut best = f64::INFINITY;
    for index in 0..profile.len() {
        let from = profile[index];
        let to = profile[(index + 1) % profile.len()];
        let edge = [to[0] - from[0], to[1] - from[1]];
        let side = edge[0] * -from[1] - edge[1] * -from[0];
        let edge_length = edge[0].hypot(edge[1]);
        if side == 0.0 || edge_length <= 1.0e-12 || (sign != 0.0 && side * sign < 0.0) {
            return None;
        }
        sign = side.signum();
        best = best.min(side.abs() / edge_length);
    }
    (best.is_finite() && best > 0.0).then_some(best)
}

/// Closest points of two segments as their distance and parameters along each, clamped to `0..=1`.
fn closest(first: [[f64; 3]; 2], second: [[f64; 3]; 2]) -> (f64, f64, f64) {
    let first_dir = sub(first[1], first[0]);
    let second_dir = sub(second[1], second[0]);
    let offset = sub(first[0], second[0]);
    let first_sq = dot(first_dir, first_dir);
    let second_sq = dot(second_dir, second_dir);
    let second_off = dot(second_dir, offset);
    if first_sq <= 1.0e-18 || second_sq <= 1.0e-18 {
        return (length(offset), 0.0, 0.0);
    }
    let dirs_dot = dot(first_dir, second_dir);
    let first_off = dot(first_dir, offset);
    let denominator = first_sq * second_sq - dirs_dot * dirs_dot;
    let mut along_first = if denominator > 1.0e-18 {
        ((dirs_dot * second_off - first_off * second_sq) / denominator).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut along_second = (dirs_dot * along_first + second_off) / second_sq;
    if along_second < 0.0 {
        along_second = 0.0;
        along_first = (-first_off / first_sq).clamp(0.0, 1.0);
    } else if along_second > 1.0 {
        along_second = 1.0;
        along_first = ((dirs_dot - first_off) / first_sq).clamp(0.0, 1.0);
    }
    let point_on = |segment: [[f64; 3]; 2], direction: [f64; 3], at: f64| {
        [0, 1, 2].map(|axis| segment[0][axis] + direction[axis] * at)
    };
    let gap = sub(
        point_on(first, first_dir, along_first),
        point_on(second, second_dir, along_second),
    );
    (length(gap), along_first, along_second)
}

/// The first pair of path segments whose tubes certainly overlap, or `None`. Cancellation is polled
/// once per segment.
pub(super) fn crossing(
    path: &[[f64; 3]],
    profile: &[[f64; 2]],
    closed: bool,
    context: &ExecutionContext,
) -> Result<Option<Overlap>, Fail> {
    let Some(radius) = inscribed_radius(profile) else {
        return Ok(None);
    };
    let needed = 2.0 * radius;
    let segments = if closed {
        path.len()
    } else {
        path.len().saturating_sub(1)
    };
    let middle = |index: usize| {
        let next = (index + 1) % path.len();
        [0, 1, 2].map(|axis| f64::midpoint(path[index][axis], path[next][axis]))
    };
    let half = |index: usize| length(sub(path[(index + 1) % path.len()], path[index])) / 2.0;
    for first in 0..segments {
        if context.is_cancelled() {
            return Err(crate::fail(
                "eval.cancelled",
                "sweep cancelled while checking for self-intersection",
            ));
        }
        let (center, reach) = (middle(first), half(first));
        for second in first + 2..segments {
            if closed && first == 0 && second + 1 == segments {
                continue;
            }
            // Cheap rejection: even the closest possible points are too far apart.
            if length(sub(center, middle(second))) - reach - half(second) >= needed {
                continue;
            }
            let (distance, along_first, along_second) = closest(
                [path[first], path[(first + 1) % path.len()]],
                [path[second], path[(second + 1) % path.len()]],
            );
            let interior = |value: f64| value > 1.0e-9 && value < 1.0 - 1.0e-9;
            if distance < needed && interior(along_first) && interior(along_second) {
                return Ok(Some(Overlap {
                    first,
                    second,
                    distance,
                    needed,
                }));
            }
        }
    }
    Ok(None)
}
