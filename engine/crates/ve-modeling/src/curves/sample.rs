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
