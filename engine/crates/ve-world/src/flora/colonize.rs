//! Space colonization after Runions et al. (2007): every node of the growing tree competes for the attraction points around it, so
//! branches fork wherever points lie in different directions, and a crown fills the way a real one does.
mod grid;
mod helpers;

use std::collections::BTreeMap;

use super::{FloraError, Skeleton, math, random::SplitMix64, wind};
use grid::NodeGrid;
use helpers::{
    add_leaves, add_node, constrain_to_crown, crown_top, droop_direction, sample_points,
    set_pipe_radii, validate,
};

/// The volume of the crown the branches grow into.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Crown {
    /// A ball.
    Sphere {
        /// Centre.
        center: [f32; 3],
        /// Radius.
        radius: f32,
    },
    /// An ellipsoid.
    Ellipsoid {
        /// Centre.
        center: [f32; 3],
        /// Radii along x, y and z.
        radii: [f32; 3],
    },
    /// A cone with its base below and its tip above.
    Cone {
        /// Centre of the base.
        base: [f32; 3],
        /// Height.
        height: f32,
        /// Radius of the base.
        radius: f32,
    },
}

/// Space-colonization settings: branches grow toward attraction points scattered inside the crown.
#[derive(Debug, Clone, PartialEq)]
pub struct Colonize {
    /// Seed.
    pub seed: u32,
    /// Height of the bare trunk before the crown, in metres.
    pub trunk_height: f32,
    /// Trunk radius at the base.
    pub trunk_radius: f32,
    /// The crown volume.
    pub crown: Crown,
    /// Attraction points (at most 50,000).
    pub points: u32,
    /// How far a node senses points, in metres.
    pub attraction_radius: f32,
    /// A point closer than this to a node is reached and removed; must be under `attraction_radius`.
    pub kill_distance: f32,
    /// Length of one growth step, in metres.
    pub step: f32,
    /// Pipe-model exponent of the radii (2 is the classic), 1.5..=3.
    pub radius_exponent: f32,
    /// Growth stops after this many iterations.
    pub max_iterations: u32,
}

/// Grows a crown by space colonization.
///
/// # Errors
/// `Cancelled`, `TooLarge`, `BadParameter` (`kill_distance` not under `attraction_radius`, too many points, exponent out of range).
#[expect(
    clippy::too_many_lines,
    reason = "colonization owns the growth transaction and keeps cancellation checks adjacent to mutations"
)]
pub fn colonize(params: &Colonize, cancel: &dyn Fn() -> bool) -> Result<Skeleton, FloraError> {
    validate(params)?;
    let mut random = SplitMix64::new(params.seed);
    let mut points = sample_points(params, &mut random)?;
    let mut skeleton = Skeleton::default();
    let mut grid = NodeGrid::new(params.attraction_radius);
    let mut has_child: Vec<bool> = Vec::new();
    let root = add_node(
        &mut skeleton,
        [0.0; 3],
        params.trunk_radius,
        u32::MAX,
        0,
        0,
        0.0,
        0.0,
        params.seed,
    );
    grid.insert(root, [0.0; 3]);
    has_child.push(false);
    let trunk_segments = math::bounded_count(params.trunk_height / params.step).max(2);
    if trunk_segments >= 200_001 {
        return Err(FloraError::TooLarge {
            nodes: usize::try_from(trunk_segments)
                .unwrap_or(usize::MAX)
                .saturating_add(1),
            max: 200_000,
            hint: "lower trunk_height or increase step".to_owned(),
        });
    }
    // the trunk leans a little and wanders, as no real one is a pole
    let lean = [
        random.signed() * 0.05 * params.trunk_height,
        random.signed() * 0.05 * params.trunk_height,
    ];
    let wander = random.unit() * 6.283_185_5;
    let mut last = root;
    for index in 1..=trunk_segments {
        if cancel() {
            return Err(FloraError::Cancelled);
        }
        let fraction = math::count_f32(index) / math::count_f32(trunk_segments);
        let (sine, cosine) = math::sin_cos(wander + fraction * 5.0);
        let drift = 0.012 * params.trunk_height * fraction * (1.0 - fraction) * 4.0;
        let position = [
            lean[0] * fraction * fraction + drift * cosine,
            params.trunk_height * fraction,
            lean[1] * fraction * fraction + drift * sine,
        ];
        last = add_node(
            &mut skeleton,
            position,
            params.trunk_radius,
            last,
            0,
            0,
            fraction,
            fraction,
            params.seed,
        );
        grid.insert(last, position);
        has_child.push(false);
        has_child[skeleton.parent[last as usize] as usize] = true;
    }
    let mut branch_id = 1_u32;
    let iteration_limit = params.max_iterations.saturating_mul(3).min(900);
    for _ in 0..iteration_limit {
        if cancel() {
            return Err(FloraError::Cancelled);
        }
        if points.is_empty() {
            break;
        }
        // each point belongs to its nearest node within reach
        let mut assignments: BTreeMap<u32, Vec<[f32; 3]>> = BTreeMap::new();
        for point in &points {
            if let Some((node, _)) = grid.nearest(&skeleton, *point, params.attraction_radius) {
                assignments.entry(node).or_default().push(*point);
            }
        }
        if assignments.is_empty() {
            break;
        }
        let mut grown = Vec::new();
        let mut given_up: std::collections::BTreeSet<[u32; 3]> = std::collections::BTreeSet::new();
        for (node, attractors) in &assignments {
            if skeleton.node_count() >= 200_000 {
                return Err(FloraError::TooLarge {
                    nodes: skeleton.node_count(),
                    max: 200_000,
                    hint: "lower points or max_iterations".to_owned(),
                });
            }
            let origin = skeleton.position[*node as usize];
            let mut direction = [0.0; 3];
            for point in attractors {
                direction = math::add(direction, math::unit(math::sub(*point, origin)));
            }
            direction = droop_direction(math::unit(direction), origin, params);
            let position = constrain_to_crown(
                math::add(origin, math::scale(direction, params.step)),
                params,
            );
            // a point that can only be reached by crowding an existing node (the crown's wall stopped the step) is given up
            if grid.any_within(&skeleton, position, params.step * 0.35) {
                given_up.extend(attractors.iter().map(|p| p.map(f32::to_bits)));
                continue;
            }
            // a node that is already a tip carries on its branch; one that has a child forks a new, thinner branch
            let tip = !has_child[*node as usize];
            let (branch, depth) = if tip {
                (
                    skeleton.branch[*node as usize],
                    skeleton.depth[*node as usize],
                )
            } else {
                branch_id = branch_id.saturating_add(1);
                (branch_id, skeleton.depth[*node as usize].saturating_add(1))
            };
            let created = add_node(
                &mut skeleton,
                position,
                params.trunk_radius * 0.02,
                *node,
                depth,
                branch,
                (position[1] / crown_top(params)).clamp(0.0, 1.0),
                1.0,
                params.seed,
            );
            skeleton.phase[created as usize] = wind::branch_phase(params.seed, branch);
            has_child.push(false);
            has_child[*node as usize] = true;
            grown.push((created, position));
        }
        for (created, position) in &grown {
            grid.insert(*created, *position);
        }
        points.retain(|point| {
            !given_up.contains(&point.map(f32::to_bits))
                && !grid.any_within(&skeleton, *point, params.kill_distance)
        });
        if grown.is_empty() {
            break;
        }
    }
    set_pipe_radii(&mut skeleton, params);
    add_leaves(&mut skeleton, params);
    Ok(skeleton)
}
