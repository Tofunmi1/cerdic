//! Keep substantial connected floor regions near the base of the geometry.
use super::{
    SurveyOptions,
    geometry::{Scene, number},
};

pub(super) fn retain(scene: &Scene, points: Vec<[f64; 3]>, o: &SurveyOptions) -> Vec<[f64; 3]> {
    let mut visited = vec![false; points.len()];
    let mut keep = vec![false; points.len()];
    let height = scene.max[1] - scene.min[1];
    for start in 0..points.len() {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        let mut region = vec![start];
        let mut next = 0;
        while next < region.len() {
            let p = points[region[next]];
            for (i, q) in points.iter().enumerate() {
                if !visited[i]
                    && (p[1] - q[1]).abs() < 0.6
                    && (p[0] - q[0]).hypot(p[2] - q[2]) <= o.spacing * 1.6
                {
                    visited[i] = true;
                    region.push(i);
                }
            }
            next += 1;
        }
        let top = region
            .iter()
            .map(|&i| points[i][1] - o.eye_height)
            .fold(f64::NEG_INFINITY, f64::max);
        let low = top <= scene.min[1] + height * 0.6 + 1e-6
            || (height < 6.0 && top <= scene.min[1] + 0.1);
        if low && number(region.len()) * o.spacing * o.spacing >= 12.0 {
            for i in region {
                keep[i] = true;
            }
        }
    }
    points
        .into_iter()
        .zip(keep)
        .filter_map(|(p, keep)| keep.then_some(p))
        .collect()
}
