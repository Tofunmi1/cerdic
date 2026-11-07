//! `inst.along_curve`: shared copies spaced evenly by length along a curve, turned to follow it.
//!
//! An open curve gets a copy at each end and the rest between (one copy sits at the start); a closed curve spaces them all the way
//! round without repeating the start. Each copy's +X axis follows the curve and its +Y stays as close to world up as the curve allows.

use glam::{DMat4, DVec3};
use ve_graph::Node;

use crate::cache::Curve3;
use crate::{Fail, fail, profile_ops::whole};

/// The point at distance `at` along the polyline (clamped to its ends; a closed curve wraps) and the direction of the segment there.
fn sample(points: &[DVec3], closed: bool, lengths: &[f64], at: f64) -> (DVec3, DVec3) {
    let total = lengths.last().copied().unwrap_or(0.0);
    let at = if closed {
        at.rem_euclid(total)
    } else {
        at.clamp(0.0, total)
    };
    let segments = if closed {
        points.len()
    } else {
        points.len() - 1
    };
    let index = lengths
        .partition_point(|&end| end < at)
        .min(segments.saturating_sub(1));
    let start = if index == 0 { 0.0 } else { lengths[index - 1] };
    let (a, b) = (points[index], points[(index + 1) % points.len()]);
    let span = (lengths[index] - start).max(1e-12);
    let t = ((at - start) / span).clamp(0.0, 1.0);
    (a.lerp(b, t), (b - a).normalize_or_zero())
}

pub(super) fn placements(node: &Node, curve: &Curve3) -> Result<Vec<DMat4>, Fail> {
    let count = whole(node, "count").map_err(|e| fail("param.type", e.to_string()))? as usize;
    let align = !matches!(node.params.get("align"), Some(ve_graph::Value::Bool(false)));
    let points: Vec<DVec3> = curve.points.iter().map(|p| DVec3::from_array(*p)).collect();
    if points.len() < 2 {
        return Err(fail(
            "graph.missing_input",
            "the curve needs at least two points",
        ));
    }
    let segments = if curve.closed {
        points.len()
    } else {
        points.len() - 1
    };
    let mut lengths = Vec::with_capacity(segments);
    let mut run = 0.0;
    for i in 0..segments {
        run += points[i].distance(points[(i + 1) % points.len()]);
        lengths.push(run);
    }
    let total = run;
    if total <= 1e-9 {
        return Err(fail("geometry.degenerate_curve", "the curve has no length"));
    }
    let station = |k: usize| {
        if curve.closed {
            total * k as f64 / count as f64
        } else if count == 1 {
            0.0
        } else {
            total * k as f64 / (count - 1) as f64
        }
    };
    let mut out = Vec::with_capacity(count);
    for k in 0..count {
        let at = station(k);
        let (position, mut tangent) = sample(&points, curve.closed, &lengths, at);
        if align {
            // The tangent at a corner is the average of the two segments, so a copy does not flip as it passes a vertex.
            let eps = total * 1e-3;
            let (ahead, _) = sample(&points, curve.closed, &lengths, at + eps);
            let (behind, _) = sample(&points, curve.closed, &lengths, at - eps);
            let smooth = (ahead - behind).normalize_or_zero();
            if smooth != DVec3::ZERO {
                tangent = smooth;
            }
            let up = if tangent.dot(DVec3::Y).abs() > 0.999 {
                DVec3::Z
            } else {
                DVec3::Y
            };
            let z = tangent.cross(up).normalize_or_zero();
            let y = z.cross(tangent);
            out.push(DMat4::from_cols(
                tangent.extend(0.0),
                y.extend(0.0),
                z.extend(0.0),
                position.extend(1.0),
            ));
        } else {
            out.push(DMat4::from_translation(position));
        }
    }
    Ok(out)
}
