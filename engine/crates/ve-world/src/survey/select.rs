//! Two-sided visibility and quality-weighted greedy coverage.
use super::{
    SurveyCamera, SurveyOptions, SurveyPlan,
    geometry::{Scene, cross, dot, length, number, sub, unit},
    sample::Sample,
};

fn visible(
    scene: &Scene,
    samples: &[Sample],
    camera: &SurveyCamera,
    o: &SurveyOptions,
) -> Vec<(usize, f64)> {
    let forward = unit(sub(camera.target, camera.position));
    let right = unit(cross(forward, [0.0, 1.0, 0.0]));
    let up = cross(right, forward);
    let tangent = (o.fov_deg.to_radians() * 0.5).tan();
    samples
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            let delta = sub(s.position, camera.position);
            let distance = length(delta);
            let depth = dot(delta, forward);
            if distance <= 1e-6
                || distance > o.max_distance
                || depth <= 0.0
                || dot(delta, right).abs() > depth * tangent
                || dot(delta, up).abs() > depth * tangent
            {
                return None;
            }
            let direction = unit(delta);
            let quality = dot(s.normal, direction).abs() / distance;
            if quality <= 1e-12
                || scene
                    .ray(camera.position, direction, distance - 1e-5)
                    .is_some()
            {
                return None;
            }
            Some((i, quality))
        })
        .collect()
}

pub(super) fn cover(
    scene: &Scene,
    samples: &[Sample],
    candidates: &[SurveyCamera],
    o: &SurveyOptions,
    aerial: bool,
) -> SurveyPlan {
    let lists: Vec<_> = candidates
        .iter()
        .map(|c| visible(scene, samples, c, o))
        .collect();
    let mut best = vec![0.0_f64; samples.len()];
    for list in &lists {
        for &(i, q) in list {
            best[i] = best[i].max(q);
        }
    }
    let reachable = best.iter().filter(|&&q| q > 0.0).count();
    let mut covered = vec![false; samples.len()];
    let mut used = vec![false; candidates.len()];
    let mut have = 0;
    let mut plan = SurveyPlan::default();
    for step in 0..o.max_views.min(candidates.len()) {
        if !aerial && (reachable == 0 || plan.coverage >= o.target_coverage) {
            break;
        }
        let good = |i: usize, q: f64| q >= (0.5 * best[i]).min(0.12);
        let chosen = if aerial {
            Some(step)
        } else {
            let mut winner = None;
            let mut gain = 0.0;
            for (k, list) in lists.iter().enumerate() {
                if used[k] {
                    continue;
                }
                let score: f64 = list
                    .iter()
                    .filter(|&&(i, q)| !covered[i] && good(i, q))
                    .map(|&(_, q)| q)
                    .sum();
                if score > gain {
                    gain = score;
                    winner = Some(k);
                }
            }
            winner
        };
        let Some(k) = chosen else {
            break;
        };
        used[k] = true;
        for &(i, q) in &lists[k] {
            if !covered[i] && good(i, q) {
                covered[i] = true;
                have += 1;
            }
        }
        let mut camera = candidates[k].clone();
        if !aerial {
            camera.label = format!("stand_{}", plan.cameras.len() + 1);
        }
        plan.cameras.push(camera);
        plan.coverage = if reachable == 0 {
            0.0
        } else {
            number(have) / number(reachable)
        };
        plan.predicted.push(plan.coverage);
    }
    plan
}
