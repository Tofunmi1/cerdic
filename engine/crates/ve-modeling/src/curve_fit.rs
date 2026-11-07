//! `curve.fit`: freehand or sketched points fitted with a smooth chain of cubic Béziers (Schneider, "An Algorithm for Automatically Fitting
//! Digitized Curves", Graphics Gems 1990), then sampled so it is a curve like any other.
//!
//! The fit keeps within `tolerance` of every input point; where the path turns sharper than `corner_angle` it is cut and each side is fitted
//! separately, so a corner stays a corner. The arithmetic is `+ - * /` and `sqrt`, so the result is the same on every platform.

use ve_graph::{Node, Value as Param};

use crate::Fail;
use crate::cache::{Curve3, Value};

type P = [f64; 3];

fn add(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn mul(a: P, s: f64) -> P {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn length(a: P) -> f64 {
    dot(a, a).sqrt()
}
fn unit(a: P) -> P {
    let l = length(a);
    if l > 0.0 { mul(a, 1.0 / l) } else { a }
}

fn bezier(c: &[P; 4], t: f64) -> P {
    let s = 1.0 - t;
    let (b0, b1, b2, b3) = (s * s * s, 3.0 * s * s * t, 3.0 * s * t * t, t * t * t);
    [0, 1, 2].map(|k| c[0][k] * b0 + c[1][k] * b1 + c[2][k] * b2 + c[3][k] * b3)
}

fn derivative(c: &[P; 4], t: f64) -> P {
    let s = 1.0 - t;
    let d = [sub(c[1], c[0]), sub(c[2], c[1]), sub(c[3], c[2])];
    [0, 1, 2].map(|k| 3.0 * (d[0][k] * s * s + 2.0 * d[1][k] * s * t + d[2][k] * t * t))
}

fn second_derivative(c: &[P; 4], t: f64) -> P {
    let s = 1.0 - t;
    let d = [
        add(sub(c[2], mul(c[1], 2.0)), c[0]),
        add(sub(c[3], mul(c[2], 2.0)), c[1]),
    ];
    [0, 1, 2].map(|k| 6.0 * (d[0][k] * s + d[1][k] * t))
}

fn chord_parameters(points: &[P]) -> Vec<f64> {
    let mut u = vec![0.0];
    for pair in points.windows(2) {
        u.push(u[u.len() - 1] + length(sub(pair[1], pair[0])));
    }
    let total = u[u.len() - 1];
    for v in &mut u {
        *v /= total;
    }
    u
}

/// The Bézier with fixed end points and end tangent directions whose handle lengths best fit the points in the least-squares sense.
fn generate(points: &[P], u: &[f64], left: P, right: P) -> [P; 4] {
    let (first, last) = (points[0], points[points.len() - 1]);
    let mut c = [[0.0; 2]; 2];
    let mut x = [0.0; 2];
    for (p, &t) in points.iter().zip(u) {
        let s = 1.0 - t;
        let (b0, b1, b2, b3) = (s * s * s, 3.0 * s * s * t, 3.0 * s * t * t, t * t * t);
        let (a1, a2) = (mul(left, b1), mul(right, b2));
        c[0][0] += dot(a1, a1);
        c[0][1] += dot(a1, a2);
        c[1][1] += dot(a2, a2);
        let rest = sub(*p, add(mul(first, b0 + b1), mul(last, b2 + b3)));
        x[0] += dot(a1, rest);
        x[1] += dot(a2, rest);
    }
    c[1][0] = c[0][1];
    let det = c[0][0] * c[1][1] - c[1][0] * c[0][1];
    let (alpha_left, alpha_right) = if det.abs() > 0.0 {
        (
            (x[0] * c[1][1] - x[1] * c[0][1]) / det,
            (c[0][0] * x[1] - c[1][0] * x[0]) / det,
        )
    } else {
        (0.0, 0.0)
    };
    let segment = length(sub(last, first));
    let epsilon = 1e-6 * segment;
    if alpha_left < epsilon || alpha_right < epsilon {
        // The fit went degenerate: the plain Wu/Barsky heuristic of a third of the chord.
        let third = segment / 3.0;
        [
            first,
            add(first, mul(left, third)),
            add(last, mul(right, third)),
            last,
        ]
    } else {
        [
            first,
            add(first, mul(left, alpha_left)),
            add(last, mul(right, alpha_right)),
            last,
        ]
    }
}

/// One Newton step per point toward the curve parameter that is closest to it.
fn reparameterize(c: &[P; 4], points: &[P], u: &[f64]) -> Vec<f64> {
    points
        .iter()
        .zip(u)
        .map(|(p, &t)| {
            let (q, d1, d2) = (bezier(c, t), derivative(c, t), second_derivative(c, t));
            let diff = sub(q, *p);
            let numerator = dot(diff, d1);
            let denominator = dot(d1, d1) + dot(diff, d2);
            if denominator.abs() < 1e-12 {
                t
            } else {
                (t - numerator / denominator).clamp(0.0, 1.0)
            }
        })
        .collect()
}

/// The largest squared distance from a point to the curve at its parameter, and which point it is.
fn worst(c: &[P; 4], points: &[P], u: &[f64]) -> (f64, usize) {
    let mut split = points.len() / 2;
    let mut largest = 0.0;
    for (i, (p, &t)) in points
        .iter()
        .zip(u)
        .enumerate()
        .skip(1)
        .take(points.len().saturating_sub(2))
    {
        let d = sub(bezier(c, t), *p);
        let squared = dot(d, d);
        if squared >= largest {
            largest = squared;
            split = i;
        }
    }
    (largest, split)
}

fn fit(points: &[P], left: P, right: P, tolerance: f64, out: &mut Vec<[P; 4]>) {
    if points.len() == 2 {
        let third = length(sub(points[1], points[0])) / 3.0;
        out.push([
            points[0],
            add(points[0], mul(left, third)),
            add(points[1], mul(right, third)),
            points[1],
        ]);
        return;
    }
    let mut u = chord_parameters(points);
    let mut c = generate(points, &u, left, right);
    let (mut error, mut split) = worst(&c, points, &u);
    let squared = tolerance * tolerance;
    if error < squared {
        out.push(c);
        return;
    }
    // Close enough to be worth refining the parameters before giving up and splitting.
    if error < squared * 16.0 {
        for _ in 0..8 {
            u = reparameterize(&c, points, &u);
            c = generate(points, &u, left, right);
            (error, split) = worst(&c, points, &u);
            if error < squared {
                out.push(c);
                return;
            }
        }
    }
    let center = unit(sub(points[split - 1], points[split + 1]));
    fit(&points[..=split], left, center, tolerance, out);
    fit(&points[split..], mul(center, -1.0), right, tolerance, out);
}

/// The Béziers fitted to one run of points (no corners inside it).
fn fit_run(points: &[P], left: Option<P>, right: Option<P>, tolerance: f64) -> Vec<[P; 4]> {
    let n = points.len();
    let left = left.unwrap_or_else(|| unit(sub(points[1.min(n - 1)], points[0])));
    let right = right.unwrap_or_else(|| unit(sub(points[n.saturating_sub(2)], points[n - 1])));
    let mut out = Vec::new();
    fit(points, left, right, tolerance, &mut out);
    out
}

fn fail(node: &Node, message: String, suggestion: &str) -> Fail {
    Box::new(
        ve_graph::Diagnostic::error("curve.fit_input", message)
            .at_node(&node.id)
            .suggest(suggestion),
    )
}

fn read_points(node: &Node) -> Result<Vec<P>, Fail> {
    let Some(Param::List(items)) = node.params.get("points") else {
        return Err(fail(
            node,
            "curve.fit needs points".to_owned(),
            "give points=[[x, y, z], ...]",
        ));
    };
    let mut out: Vec<P> = Vec::with_capacity(items.len());
    for item in items {
        let Param::List(xyz) = item else { continue };
        let v: Vec<f64> = xyz.iter().filter_map(Param::as_f64).collect();
        if v.len() == 3 {
            let p = [v[0], v[1], v[2]];
            // A repeated point (a pen resting on the paper) adds nothing and breaks the chord lengths.
            if out.last().is_none_or(|last| length(sub(p, *last)) > 1e-9) {
                out.push(p);
            }
        }
    }
    Ok(out)
}

pub(crate) fn build(node: &Node) -> Result<Value, Fail> {
    let mut points = read_points(node)?;
    let number = |name: &str, default: f64| {
        node.params
            .get(name)
            .and_then(Param::as_f64)
            .unwrap_or(default)
    };
    let tolerance = number("tolerance", 0.02);
    let corner = number("corner_angle", 60.0_f64.to_radians());
    let steps = number("steps", 12.0) as usize;
    let closed = matches!(node.params.get("closed"), Some(Param::Bool(true)));
    if closed && points.len() > 2 && length(sub(points[0], points[points.len() - 1])) <= 1e-9 {
        points.pop();
    }
    if points.len() < 2 || (closed && points.len() < 3) {
        return Err(fail(
            node,
            format!(
                "only {} distinct point(s) to fit; a curve needs at least {}",
                points.len(),
                if closed { 3 } else { 2 }
            ),
            "give more points, spaced apart (repeated points are ignored)",
        ));
    }
    let cos_corner = corner.cos();
    let n = points.len();
    // Where the path turns sharper than the corner angle it is cut.
    let turns = |i: usize| {
        let (a, b) = (
            unit(sub(points[i], points[(i + n - 1) % n])),
            unit(sub(points[(i + 1) % n], points[i])),
        );
        dot(a, b) < cos_corner
    };
    let cuts: Vec<usize> = (0..n)
        .filter(|&i| (closed || (i > 0 && i + 1 < n)) && turns(i))
        .collect();
    let mut pieces: Vec<[P; 4]> = Vec::new();
    if closed && cuts.is_empty() {
        // One smooth loop: the path is fitted from the first point round to it again, with the same tangent at both ends.
        let mut loop_points = points.clone();
        loop_points.push(points[0]);
        let tangent = unit(sub(points[1], points[n - 1]));
        pieces = fit_run(
            &loop_points,
            Some(tangent),
            Some(mul(tangent, -1.0)),
            tolerance,
        );
    } else {
        let mut bounds: Vec<usize> = Vec::new();
        if !closed {
            bounds.push(0);
        }
        bounds.extend(cuts.iter().copied());
        if !closed {
            bounds.push(n - 1);
        }
        let runs = if closed { cuts.len() } else { bounds.len() - 1 };
        for r in 0..runs {
            let (start, end) = if closed {
                (cuts[r], cuts[(r + 1) % cuts.len()])
            } else {
                (bounds[r], bounds[r + 1])
            };
            let mut run: Vec<P> = Vec::new();
            let mut i = start;
            loop {
                run.push(points[i % n]);
                if i % n == end % n && !run.is_empty() && run.len() > 1 {
                    break;
                }
                i += 1;
                if i > start + n {
                    break;
                }
            }
            if run.len() >= 2 {
                pieces.extend(fit_run(&run, None, None, tolerance));
            }
        }
    }
    let mut samples: Vec<P> = Vec::new();
    for c in &pieces {
        for k in 0..=steps {
            let p = bezier(c, k as f64 / steps as f64);
            if samples
                .last()
                .is_none_or(|last| length(sub(p, *last)) > 1e-12)
            {
                samples.push(p);
            }
        }
    }
    if closed && samples.len() > 1 && length(sub(samples[0], samples[samples.len() - 1])) <= 1e-9 {
        samples.pop();
    }
    Ok(Value::curve(Curve3 {
        points: samples,
        closed,
    }))
}
