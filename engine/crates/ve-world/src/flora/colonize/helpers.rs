//! Validation and shared data operations for space colonization.

use super::super::{Colonize, Crown, FloraError, Leaf, Skeleton, math, random::SplitMix64, wind};

pub(super) fn validate(params: &Colonize) -> Result<(), FloraError> {
    if params.points > 50_000 || params.points == 0 {
        return Err(bad("points", "use 1 to 50,000 attraction points"));
    }
    if !params.trunk_height.is_finite()
        || params.trunk_height <= 0.0
        || !params.trunk_radius.is_finite()
        || params.trunk_radius <= 0.0
    {
        return Err(bad(
            "trunk_height",
            "trunk dimensions must be finite and positive",
        ));
    }
    if !params.attraction_radius.is_finite()
        || !params.kill_distance.is_finite()
        || params.kill_distance <= 0.0
        || params.kill_distance >= params.attraction_radius
    {
        return Err(bad(
            "kill_distance",
            "must be positive and smaller than attraction_radius",
        ));
    }
    if !params.step.is_finite() || params.step <= 0.0 {
        return Err(bad("step", "must be finite and positive"));
    }
    if !params.radius_exponent.is_finite() || !(1.5..=3.0).contains(&params.radius_exponent) {
        return Err(bad("radius_exponent", "use a value from 1.5 to 3"));
    }
    if params.max_iterations == 0 {
        return Err(bad("max_iterations", "must be positive"));
    }
    Ok(())
}

pub(super) fn sample_points(
    params: &Colonize,
    random: &mut SplitMix64,
) -> Result<Vec<[f32; 3]>, FloraError> {
    let mut points = Vec::with_capacity(params.points as usize);
    let mut attempts = 0_u32;
    let attempt_limit = params.points.saturating_mul(40).max(100);
    while points.len() < params.points as usize && attempts < attempt_limit {
        attempts += 1;
        let candidate = match params.crown {
            Crown::Sphere { center, radius } => [
                center[0] + random.signed() * radius,
                center[1] + random.signed() * radius,
                center[2] + random.signed() * radius,
            ],
            Crown::Ellipsoid { center, radii } => [
                center[0] + random.signed() * radii[0],
                center[1] + random.signed() * radii[1],
                center[2] + random.signed() * radii[2],
            ],
            Crown::Cone {
                base,
                height,
                radius,
            } => {
                let y = random.unit();
                let r = radius * (1.0 - y) * random.unit().sqrt();
                let angle = random.unit() * 6.283_185_5;
                let (sine, cosine) = math::sin_cos(angle);
                [
                    base[0] + r * cosine,
                    base[1] + y * height,
                    base[2] + r * sine,
                ]
            }
        };
        if inside_crown(candidate, params) {
            points.push(candidate);
        }
    }
    if points.len() < params.points as usize {
        return Err(bad(
            "crown",
            "the crown dimensions must be finite and positive",
        ));
    }
    Ok(points)
}

fn inside_crown(point: [f32; 3], params: &Colonize) -> bool {
    match params.crown {
        Crown::Sphere { center, radius } => {
            radius.is_finite() && radius > 0.0 && math::length(math::sub(point, center)) <= radius
        }
        Crown::Ellipsoid { center, radii } => {
            if radii
                .iter()
                .any(|radius| !radius.is_finite() || *radius <= 0.0)
            {
                return false;
            }
            let delta = math::sub(point, center);
            (delta[0] / radii[0]).powi(2)
                + (delta[1] / radii[1]).powi(2)
                + (delta[2] / radii[2]).powi(2)
                <= 1.0
        }
        Crown::Cone {
            base,
            height,
            radius,
        } => {
            if !height.is_finite() || !radius.is_finite() || height <= 0.0 || radius <= 0.0 {
                return false;
            }
            let y = (point[1] - base[1]) / height;
            if !(0.0..=1.0).contains(&y) {
                return false;
            }
            let local_radius = radius * (1.0 - y);
            (point[0] - base[0]).powi(2) + (point[2] - base[2]).powi(2)
                <= local_radius * local_radius
        }
    }
}

pub(super) fn set_pipe_radii(skeleton: &mut Skeleton, params: &Colonize) {
    let mut children = vec![Vec::new(); skeleton.node_count()];
    for node in 1..skeleton.node_count() {
        children[skeleton.parent[node] as usize].push(node);
    }
    for node in (0..skeleton.node_count()).rev() {
        if children[node].is_empty() {
            skeleton.radius[node] = (params.trunk_radius * 0.035).max(0.001);
        } else {
            let sum = children[node]
                .iter()
                .map(|child| skeleton.radius[*child].powf(params.radius_exponent))
                .sum::<f32>();
            skeleton.radius[node] = sum.powf(1.0 / params.radius_exponent);
        }
    }
    let root_radius = skeleton.radius[0];
    if root_radius > 0.0 {
        let scale = params.trunk_radius / root_radius;
        for radius in &mut skeleton.radius {
            *radius *= scale;
        }
    }
}

pub(super) fn droop_direction(
    direction: [f32; 3],
    origin: [f32; 3],
    params: &Colonize,
) -> [f32; 3] {
    if matches!(params.crown, Crown::Sphere { .. })
        && params.trunk_height < crown_top(params) * 0.08
    {
        return math::unit(math::add(direction, [0.0, 1.4, 0.0]));
    }
    let Crown::Ellipsoid { radii, .. } = params.crown else {
        return direction;
    };
    let width = radii[0].max(radii[2]);
    if width <= radii[1] * 1.5 {
        return direction;
    }
    let reach = ((origin[0] / radii[0]).powi(2) + (origin[2] / radii[2]).powi(2)).sqrt();
    math::unit(math::add(
        direction,
        [0.0, -0.38 * reach.clamp(0.0, 1.0), 0.0],
    ))
}

pub(super) fn constrain_to_crown(mut point: [f32; 3], params: &Colonize) -> [f32; 3] {
    match params.crown {
        Crown::Sphere { center, radius } => {
            let delta = math::sub(point, center);
            let limit = radius + params.step;
            let distance = math::length(delta);
            if distance > limit {
                point = math::add(center, math::scale(delta, limit / distance));
            }
        }
        Crown::Ellipsoid { center, radii } => {
            let delta = math::sub(point, center);
            let x = delta[0] / (radii[0] + params.step);
            let y = delta[1] / (radii[1] + params.step);
            let z = delta[2] / (radii[2] + params.step);
            let distance = (x * x + y * y + z * z).sqrt();
            if distance > 1.0 {
                point = math::add(
                    center,
                    [
                        x * (radii[0] + params.step) / distance,
                        y * (radii[1] + params.step) / distance,
                        z * (radii[2] + params.step) / distance,
                    ],
                );
            }
        }
        Crown::Cone {
            base,
            height,
            radius,
        } => {
            let top = base[1] + height + params.step;
            point[1] = point[1].min(top);
            let vertical = ((point[1] - base[1]) / height).clamp(0.0, 1.0);
            let radial_limit = radius * (1.0 - vertical) + params.step;
            let dx = point[0] - base[0];
            let dz = point[2] - base[2];
            let radial = (dx * dx + dz * dz).sqrt();
            if radial > radial_limit {
                point[0] = base[0] + dx * radial_limit / radial;
                point[2] = base[2] + dz * radial_limit / radial;
            }
        }
    }
    point
}

pub(super) fn add_node(
    skeleton: &mut Skeleton,
    position: [f32; 3],
    radius: f32,
    parent: u32,
    depth: u8,
    branch: u32,
    height: f32,
    fraction: f32,
    seed: u32,
) -> u32 {
    let id = u32::try_from(skeleton.node_count()).unwrap_or(u32::MAX);
    skeleton.position.push(position);
    skeleton.radius.push(radius);
    skeleton.parent.push(parent);
    skeleton.depth.push(depth);
    wind::append(
        skeleton,
        branch,
        depth,
        height,
        fraction,
        wind::branch_phase(seed, branch),
    );
    id
}

pub(super) fn add_leaves(skeleton: &mut Skeleton, params: &Colonize) {
    let node_count = skeleton.node_count();
    // leaves sit on the thin twigs: wherever a branch has thinned to a few per cent of the trunk
    let twig = params.trunk_radius * 0.09;
    for node in 1..node_count {
        if skeleton.radius[node] > twig || skeleton.parent[node] == u32::MAX {
            continue;
        }
        let parent = skeleton.position[skeleton.parent[node] as usize];
        let direction = math::unit(math::sub(skeleton.position[node], parent));
        let side = math::perpendicular(direction);
        let cluster_count = match params.crown {
            // a conifer's crown narrows to a spire, where there are few twigs, so each carries more needles
            Crown::Cone { base, height, .. }
                if skeleton.position[node][1] >= base[1] + height * 0.6 =>
            {
                8
            }
            Crown::Cone { .. } => 3,
            _ => 2,
        };
        for cluster in 0..cluster_count {
            let sign = if cluster == 0 { -1.0 } else { 1.0 };
            let angle = math::count_f32(cluster) * 2.399_963_1;
            let radial = math::rotate(side, direction, angle);
            let offset = math::scale(radial, sign * params.step * 0.22);
            skeleton.leaves.push(Leaf {
                position: math::add(skeleton.position[node], offset),
                direction: math::unit(math::add(
                    math::rotate(direction, radial, sign * 0.48),
                    [0.0, if params.seed & 1 == 0 { 0.18 } else { -0.18 }, 0.0],
                )),
                size: params.step * 0.45,
                node: u32::try_from(node).unwrap_or(u32::MAX),
            });
        }
    }
}

pub(super) fn crown_top(params: &Colonize) -> f32 {
    match params.crown {
        Crown::Sphere { center, radius } => center[1] + radius,
        Crown::Ellipsoid { center, radii } => center[1] + radii[1],
        Crown::Cone { base, height, .. } => base[1] + height,
    }
    .max(params.trunk_height + 0.01)
}

pub(super) fn bad(name: &'static str, why: &str) -> FloraError {
    FloraError::BadParameter {
        name,
        why: why.to_owned(),
    }
}
