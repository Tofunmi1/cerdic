//! Curve generators and modifiers beyond the first three: a smooth spline through points, a helix, rounded corners,
//! even resampling and reversal. All arithmetic is `+ - * /` and `sqrt` (the helix also uses `sin`/`cos`), so native
//! and WASM agree.
#![expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "sample and control-point counts are capped far below 2^52 and index arithmetic stays in range"
)]
use crate::{
    Fail,
    cache::{Curve3, Value},
    fail,
};
use std::rc::Rc;
use ve_graph::{Node, Value as Param};

/// Most samples any curve node may produce.
pub(super) const MAX_SAMPLES: usize = 8192;

type P3 = [f64; 3];

fn lerp(a: P3, b: P3, t: f64) -> P3 {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn dist(a: P3, b: P3) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn int(node: &Node, name: &str, default: usize) -> usize {
    match node.params.get(name) {
        Some(Param::Int(value)) => usize::try_from(*value).unwrap_or(default),
        _ => default,
    }
}

fn num(node: &Node, name: &str, default: f64) -> f64 {
    node.params
        .get(name)
        .and_then(Param::as_f64)
        .unwrap_or(default)
}

fn input_curve(inputs: &[Rc<Value>], op: &str) -> Result<Curve3, Fail> {
    inputs
        .first()
        .and_then(|value| value.curve.clone())
        .ok_or_else(|| fail("graph.missing_input", format!("{op} needs a curve")))
}

fn budget(count: usize, op: &str) -> Result<(), Fail> {
    if count > MAX_SAMPLES {
        return Err(fail(
            "budget.curve_samples",
            format!("{op} would make {count} samples; at most {MAX_SAMPLES}"),
        ));
    }
    Ok(())
}

/// How many samples a node of this family produces from inputs of the given sizes (an upper bound).
pub(super) fn estimate(node: &Node, input_samples: usize) -> usize {
    match node.op.family().as_str() {
        "curve.spline" => {
            let points = match node.params.get("points") {
                Some(Param::List(points)) => points.len(),
                _ => 0,
            };
            points
                .saturating_mul(int(node, "steps", 8))
                .saturating_add(1)
        }
        "curve.helix" => int(node, "turns", 1)
            .saturating_mul(int(node, "segments_per_turn", 16))
            .saturating_add(1),
        "curve.resample" => int(node, "count", 16),
        "curve.fillet" => input_samples.saturating_mul(int(node, "steps", 6).saturating_add(1)),
        _ => input_samples,
    }
}

/// Whether the family is one of these.
pub(super) fn handles(family: &str) -> bool {
    matches!(
        family,
        "curve.spline" | "curve.helix" | "curve.resample" | "curve.fillet" | "curve.reverse"
    )
}

pub(super) fn build(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let family = node.op.family();
    let curve = match family.as_str() {
        "curve.spline" => spline(node)?,
        "curve.helix" => helix(node)?,
        "curve.resample" => resample(node, &input_curve(inputs, "curve.resample")?)?,
        "curve.fillet" => fillet(node, &input_curve(inputs, "curve.fillet")?)?,
        "curve.reverse" => {
            let mut curve = input_curve(inputs, "curve.reverse")?;
            curve.points.reverse();
            curve
        }
        other => {
            return Err(fail(
                "graph.unknown_op",
                format!("{other} is not a curve tool"),
            ));
        }
    };
    if curve
        .points
        .windows(2)
        .any(|pair| dist(pair[0], pair[1]) <= 1.0e-9)
    {
        return Err(fail(
            "geometry.degenerate_curve",
            "the curve has a repeated consecutive point; remove the duplicate",
        ));
    }
    Ok(Value::curve(curve))
}

fn points(node: &Node) -> Result<Vec<P3>, Fail> {
    let Some(Param::List(items)) = node.params.get("points") else {
        return Err(fail("param.type", "curve points are missing"));
    };
    items
        .iter()
        .map(|item| match item {
            Param::List(c) if c.len() == 3 => {
                let get = |i: usize| {
                    c[i].as_f64()
                        .ok_or_else(|| fail("param.type", "point component is not numeric"))
                };
                Ok([get(0)?, get(1)?, get(2)?])
            }
            _ => Err(fail("param.type", "expected a 3D point")),
        })
        .collect()
}

/// Centripetal Catmull-Rom through the points (Barry-Goldman form): passes through every point, never overshoots or
/// loops within a span, and uses only `+ - * /` and `sqrt`.
fn spline(node: &Node) -> Result<Curve3, Fail> {
    let controls = points(node)?;
    let closed = matches!(node.params.get("closed"), Some(Param::Bool(true)));
    let steps = int(node, "steps", 8).clamp(1, 64);
    if controls.len() < 2 || (closed && controls.len() < 3) {
        return Err(fail(
            "geometry.invalid_curve",
            "a spline needs 2 points (3 when closed)",
        ));
    }
    let n = controls.len();
    budget(n.saturating_mul(steps).saturating_add(1), "curve.spline")?;
    let at = |i: isize| -> P3 {
        if closed {
            controls[i.rem_euclid(n as isize) as usize]
        } else if i < 0 {
            // Mirror the first point across the second so the end tangent is natural.
            let (a, b) = (controls[0], controls[1]);
            [2.0 * a[0] - b[0], 2.0 * a[1] - b[1], 2.0 * a[2] - b[2]]
        } else if i as usize >= n {
            let (a, b) = (controls[n - 1], controls[n - 2]);
            [2.0 * a[0] - b[0], 2.0 * a[1] - b[1], 2.0 * a[2] - b[2]]
        } else {
            controls[i as usize]
        }
    };
    let spans = if closed { n } else { n - 1 };
    let mut out = Vec::with_capacity(spans * steps + 1);
    for span in 0..spans {
        let s = span as isize;
        let (p0, p1, p2, p3) = (at(s - 1), at(s), at(s + 1), at(s + 2));
        // Knot spacing is the square root of the chord length (alpha = 0.5), floored so coincident points stay finite.
        let t1 = dist(p0, p1).sqrt().max(1e-6);
        let t2 = t1 + dist(p1, p2).sqrt().max(1e-6);
        let t3 = t2 + dist(p2, p3).sqrt().max(1e-6);
        let t0 = 0.0;
        for step in 0..steps {
            let t = t1 + (t2 - t1) * step as f64 / steps as f64;
            let a1 = lerp(p0, p1, (t - t0) / (t1 - t0));
            let a2 = lerp(p1, p2, (t - t1) / (t2 - t1));
            let a3 = lerp(p2, p3, (t - t2) / (t3 - t2));
            let b1 = lerp(a1, a2, (t - t0) / (t2 - t0));
            let b2 = lerp(a2, a3, (t - t1) / (t3 - t1));
            out.push(lerp(b1, b2, (t - t1) / (t2 - t1)));
        }
    }
    if !closed {
        out.push(controls[n - 1]);
    }
    Ok(Curve3 {
        points: out,
        closed,
    })
}

/// A helix (or spiral, when `end_radius` differs) about the vertical axis through `center`, rising from its y.
fn helix(node: &Node) -> Result<Curve3, Fail> {
    let center = match node.params.get("center") {
        Some(Param::List(c)) if c.len() == 3 => [
            c[0].as_f64().unwrap_or(0.0),
            c[1].as_f64().unwrap_or(0.0),
            c[2].as_f64().unwrap_or(0.0),
        ],
        _ => [0.0; 3],
    };
    let radius = num(node, "radius", 0.5);
    let end_radius = num(node, "end_radius", radius);
    let pitch = num(node, "pitch", 0.2);
    let turns = int(node, "turns", 3);
    let per_turn = int(node, "segments_per_turn", 16).clamp(4, 128);
    if radius <= 0.0 || end_radius <= 0.0 || turns == 0 {
        return Err(fail(
            "geometry.invalid_curve",
            "a helix needs positive radii and at least one turn",
        ));
    }
    let total = turns * per_turn;
    budget(total + 1, "curve.helix")?;
    let points = (0..=total)
        .map(|i| {
            let t = i as f64 / total as f64;
            let angle = std::f64::consts::TAU * turns as f64 * t;
            let r = radius + (end_radius - radius) * t;
            [
                center[0] + r * angle.cos(),
                center[1] + pitch * turns as f64 * t,
                center[2] + r * angle.sin(),
            ]
        })
        .collect();
    Ok(Curve3 {
        points,
        closed: false,
    })
}

/// Evenly spaced samples along the curve's length.
fn resample(node: &Node, curve: &Curve3) -> Result<Curve3, Fail> {
    let count = int(node, "count", 16).clamp(2, MAX_SAMPLES);
    let mut path = curve.points.clone();
    if curve.closed {
        path.push(path[0]);
    }
    let mut lengths = vec![0.0];
    for pair in path.windows(2) {
        lengths.push(lengths.last().copied().unwrap_or(0.0) + dist(pair[0], pair[1]));
    }
    let total = lengths.last().copied().unwrap_or(0.0);
    if total <= 1e-9 {
        return Err(fail(
            "geometry.degenerate_curve",
            "the curve has no length to resample",
        ));
    }
    let samples = if curve.closed { count } else { count - 1 };
    let mut out = Vec::with_capacity(count);
    let mut segment = 0;
    for i in 0..=samples {
        if curve.closed && i == samples {
            break;
        }
        let target = total * i as f64 / samples as f64;
        while segment + 2 < lengths.len() && lengths[segment + 1] < target {
            segment += 1;
        }
        let (a, b) = (lengths[segment], lengths[segment + 1]);
        let t = if b - a > 1e-12 {
            ((target - a) / (b - a)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        out.push(lerp(path[segment], path[segment + 1], t));
    }
    Ok(Curve3 {
        points: out,
        closed: curve.closed,
    })
}

/// Rounds every interior corner: the corner is replaced by a quadratic curve that starts `distance` before it and ends
/// `distance` after it (shortened to half of a short neighbouring segment), with the corner as its control point.
fn fillet(node: &Node, curve: &Curve3) -> Result<Curve3, Fail> {
    let distance = num(node, "distance", 0.1);
    let steps = int(node, "steps", 6).clamp(1, 32);
    if distance <= 0.0 {
        return Err(fail("param.range", "curve.fillet distance must be above 0"));
    }
    let n = curve.points.len();
    budget(n.saturating_mul(steps + 1), "curve.fillet")?;
    let mut out: Vec<P3> = Vec::new();
    for i in 0..n {
        let corner = curve.points[i];
        let interior = curve.closed || (i > 0 && i + 1 < n);
        if !interior {
            out.push(corner);
            continue;
        }
        let (before, after) = (curve.points[(i + n - 1) % n], curve.points[(i + 1) % n]);
        let (to_before, to_after) = (dist(corner, before), dist(corner, after));
        let reach = distance.min(to_before / 2.0).min(to_after / 2.0);
        let (start, end) = (
            lerp(corner, before, reach / to_before),
            lerp(corner, after, reach / to_after),
        );
        for step in 0..=steps {
            let t = step as f64 / steps as f64;
            out.push(lerp(lerp(start, corner, t), lerp(corner, end, t), t));
        }
    }
    // Where two neighbouring corners both reach the middle of their shared segment, the end of one is the start of the next.
    let mut points: Vec<P3> = Vec::with_capacity(out.len());
    for point in out {
        if points.last().is_none_or(|last| dist(*last, point) > 1.0e-9) {
            points.push(point);
        }
    }
    if curve.closed && points.len() > 1 && dist(points[0], points[points.len() - 1]) <= 1.0e-9 {
        points.pop();
    }
    Ok(Curve3 {
        points,
        closed: curve.closed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lerp_and_dist_agree() {
        let a = [0.0, 0.0, 0.0];
        let b = [3.0, 4.0, 0.0];
        assert!((dist(a, b) - 5.0).abs() < 1e-12);
        let middle = lerp(a, b, 0.5);
        assert!(
            (middle[0] - 1.5).abs() < 1e-12
                && (middle[1] - 2.0).abs() < 1e-12
                && middle[2].abs() < 1e-12
        );
    }
}
