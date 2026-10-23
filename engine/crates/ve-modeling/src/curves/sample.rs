//! Deterministic cubic Bézier sampling.

pub(super) fn bezier(points: &[[f64; 3]], steps: usize) -> Result<Vec<[f64; 3]>, crate::Fail> {
    if points.len() != 4 || steps == 0 || steps > 512 {
        return Err(crate::fail(
            "geometry.invalid_curve",
            "cubic Bézier needs four controls and 1..512 steps",
        ));
    }
    let mut sampled = Vec::with_capacity(steps + 1);
    let step_count = u32::try_from(steps)
        .map_err(|_| crate::fail("geometry.invalid_curve", "Bézier step count overflow"))?;
    for index in 0..=step_count {
        let t = f64::from(index) / f64::from(step_count);
        let u = 1.0 - t;
        let weights = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
        let mut point = [0.0; 3];
        for (control, weight) in points.iter().zip(weights) {
            for axis in 0..3 {
                point[axis] += control[axis] * weight;
            }
        }
        sampled.push(point);
    }
    Ok(sampled)
}

/// Samples a rope hanging as a catenary under gravity along -Y.
pub(super) fn hang(
    from: [f64; 3],
    to: [f64; 3],
    slack: f64,
    points: usize,
) -> Result<Vec<[f64; 3]>, crate::Fail> {
    if !(3..=512).contains(&points) || !slack.is_finite() || !(0.0..=2.0).contains(&slack) {
        return Err(crate::fail(
            "geometry.invalid_curve",
            "invalid hanging curve parameters",
        ));
    }
    let delta = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
    let distance = delta.iter().map(|v| v * v).sum::<f64>().sqrt();
    if distance <= 1.0e-9 {
        return Err(crate::fail(
            "geometry.invalid_curve",
            "curve endpoints must be distinct",
        ));
    }
    let length = distance * (1.0 + slack);
    let h = delta[0].hypot(delta[2]);
    let mut sampled = Vec::with_capacity(points);
    if slack == 0.0 || length - distance < 1.0e-9 || h < 1.0e-9 {
        for index in 0..points {
            let t = index as f64 / (points - 1) as f64;
            sampled.push([
                from[0] + delta[0] * t,
                from[1] + delta[1] * t,
                from[2] + delta[2] * t,
            ]);
        }
        return Ok(sampled);
    }

    let horizontal_arc = (length * length - delta[1] * delta[1]).sqrt();
    let a = solve_catenary(h, horizontal_arc)?;
    let q = h / (2.0 * a);
    let shift = (delta[1] / horizontal_arc).asinh();
    let start_cosh = (-q + shift).cosh();
    for index in 0..points {
        let t = index as f64 / (points - 1) as f64;
        let x = h * t;
        let y = from[1] + a * ((x / a - q + shift).cosh() - start_cosh);
        sampled.push([from[0] + delta[0] * t, y, from[2] + delta[2] * t]);
    }
    sampled[0] = from;
    if let Some(last) = sampled.last_mut() {
        *last = to;
    }
    Ok(sampled)
}

fn solve_catenary(h: f64, horizontal_arc: f64) -> Result<f64, crate::Fail> {
    if !h.is_finite() || !horizontal_arc.is_finite() || h <= 0.0 || horizontal_arc <= h {
        return Err(crate::fail(
            "geometry.invalid_curve",
            "catenary length is not solvable",
        ));
    }
    let value = |a: f64| {
        let x = h / (2.0 * a);
        if x > 700.0 {
            f64::INFINITY
        } else {
            2.0 * a * x.sinh()
        }
    };
    let mut low = h * 1.0e-12;
    let mut high = h;
    for _ in 0..200 {
        let mid = (low + high) * 0.5;
        if value(mid) > horizontal_arc {
            low = mid;
        } else {
            high = mid;
        }
    }
    Ok((low + high) * 0.5)
}

#[cfg(test)]
mod hang_tests {
    use super::hang;

    #[test]
    fn sloped_catenary_samples_have_the_requested_rope_length() {
        let from = [0.0, 4.0, -1.0];
        let to = [8.0, 1.0, 3.0];
        let slack = 0.3;
        let curve = hang(from, to, slack, 512);
        assert!(curve.is_ok());
        let points = match curve {
            Ok(points) => points,
            Err(_) => return,
        };
        let length: f64 = points
            .windows(2)
            .map(|pair| {
                (0..3)
                    .map(|axis| (pair[1][axis] - pair[0][axis]).powi(2))
                    .sum::<f64>()
                    .sqrt()
            })
            .sum();
        let straight = (0..3)
            .map(|axis| (to[axis] - from[axis]).powi(2))
            .sum::<f64>()
            .sqrt();
        let expected = straight * (1.0 + slack);
        assert!(
            (length - expected).abs() / expected < 0.002,
            "sampled {length}, expected {expected}"
        );
    }
}

pub(super) fn circle(
    center: [f64; 3],
    radius: f64,
    segments: usize,
) -> Result<Vec<[f64; 3]>, crate::Fail> {
    if !(8..=512).contains(&segments) || !radius.is_finite() || radius <= 0.0 {
        return Err(crate::fail(
            "geometry.invalid_curve",
            "circle needs a positive radius and 8..512 segments",
        ));
    }
    let segment_count = u32::try_from(segments)
        .map_err(|_| crate::fail("geometry.invalid_curve", "circle segment count overflow"))?;
    Ok((0..segment_count)
        .map(|index| {
            let angle = std::f64::consts::TAU * f64::from(index) / f64::from(segment_count);
            [
                center[0] + radius * angle.cos(),
                center[1],
                center[2] + radius * angle.sin(),
            ]
        })
        .collect())
}
