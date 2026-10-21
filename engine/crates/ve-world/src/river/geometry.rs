//! Shared path sampling and distance queries for river and road corridors.
use super::{Ground, Path, RiverError};

#[derive(Clone, Copy)]
pub(super) struct Sample {
    pub point: [f32; 2],
    pub distance: f32,
}

pub(super) fn path_samples(path: &Path, spacing: f32) -> Result<Vec<Sample>, RiverError> {
    let points = &path.points;
    if points.len() < 2
        || points
            .iter()
            .any(|point| !point[0].is_finite() || !point[1].is_finite())
    {
        return Err(RiverError::BadPath {
            why: "the path needs at least two finite points".into(),
        });
    }
    if !points
        .windows(2)
        .any(|pair| distance(pair[0], pair[1]) > 1.0e-5)
    {
        return Err(RiverError::BadPath {
            why: "the path needs two distinct points".into(),
        });
    }
    let mut dense = Vec::new();
    for segment in 0..points.len() - 1 {
        let p0 = points[segment.saturating_sub(1)];
        let p1 = points[segment];
        let p2 = points[segment + 1];
        let p3 = points[(segment + 2).min(points.len() - 1)];
        let step_count =
            crate::num::rounded_offset((distance(p1, p2) / spacing.max(0.05)).ceil()).max(1);
        let steps = usize::try_from(step_count).unwrap_or(usize::MAX);
        for step in 0..steps {
            dense.push(catmull(
                p0,
                p1,
                p2,
                p3,
                crate::num::cell_f32(step) / crate::num::cell_f32(steps),
            ));
        }
    }
    dense.push(*points.last().ok_or_else(|| RiverError::BadPath {
        why: "empty path".into(),
    })?);
    let mut output = Vec::with_capacity(dense.len());
    let mut travelled = 0.0;
    for index in 0..dense.len() {
        if index > 0 {
            travelled += distance(dense[index - 1], dense[index]);
        }
        output.push(Sample {
            point: dense[index],
            distance: travelled,
        });
    }
    Ok(output)
}

pub(super) fn nearest(samples: &[Sample], point: [f32; 2]) -> (f32, [f32; 2], f32) {
    let mut best_distance = f32::INFINITY;
    let mut best_tangent = [0.0, 0.0];
    let mut best_along = 0.0;
    for pair in samples.windows(2) {
        let delta = [
            pair[1].point[0] - pair[0].point[0],
            pair[1].point[1] - pair[0].point[1],
        ];
        let length_squared = delta[0] * delta[0] + delta[1] * delta[1];
        if length_squared <= f32::EPSILON {
            continue;
        }
        let relative = [point[0] - pair[0].point[0], point[1] - pair[0].point[1]];
        let t =
            ((relative[0] * delta[0] + relative[1] * delta[1]) / length_squared).clamp(0.0, 1.0);
        let closest = [
            pair[0].point[0] + t * delta[0],
            pair[0].point[1] + t * delta[1],
        ];
        let candidate = distance(point, closest);
        if candidate < best_distance {
            best_distance = candidate;
            best_tangent = unit(delta);
            best_along = pair[0].distance + t * length_squared.sqrt();
        }
    }
    (best_distance, best_tangent, best_along)
}

pub(super) fn overlaps_grid(samples: &[Sample], ground: &Ground<'_>) -> bool {
    samples.iter().any(|sample| {
        point_in_grid(
            sample.point,
            ground.width,
            ground.depth,
            ground.cell,
            ground.origin,
        )
    })
}

fn point_in_grid(point: [f32; 2], width: usize, depth: usize, cell: f32, origin: [f32; 2]) -> bool {
    let x = (point[0] - origin[0]) / cell;
    let z = (point[1] - origin[1]) / cell;
    x >= 0.0
        && z >= 0.0
        && x <= crate::num::cell_f32(width - 1)
        && z <= crate::num::cell_f32(depth - 1)
}

pub(super) fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    let dx = a[0] - b[0];
    let dz = a[1] - b[1];
    (dx * dx + dz * dz).sqrt()
}

pub(super) fn unit(vector: [f32; 2]) -> [f32; 2] {
    let length = (vector[0] * vector[0] + vector[1] * vector[1]).sqrt();
    if length > f32::EPSILON {
        [vector[0] / length, vector[1] / length]
    } else {
        [0.0, 0.0]
    }
}

fn catmull(p0: [f32; 2], p1: [f32; 2], p2: [f32; 2], p3: [f32; 2], t: f32) -> [f32; 2] {
    let t2 = t * t;
    let t3 = t2 * t;
    [
        0.5 * (2.0 * p1[0]
            + (p2[0] - p0[0]) * t
            + (2.0 * p0[0] - 5.0 * p1[0] + 4.0 * p2[0] - p3[0]) * t2
            + (3.0 * p1[0] - p0[0] - 3.0 * p2[0] + p3[0]) * t3),
        0.5 * (2.0 * p1[1]
            + (p2[1] - p0[1]) * t
            + (2.0 * p0[1] - 5.0 * p1[1] + 4.0 * p2[1] - p3[1]) * t2
            + (3.0 * p1[1] - p0[1] - 3.0 * p2[1] + p3[1]) * t3),
    ]
}
