//! Parametric trees after Weber and Penn (1995).
use super::{FloraError, Skeleton, math, random::SplitMix64, wind};

/// One level of branching (level 0 is the trunk's own growth, level 1 the first branches, and so on).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Level {
    /// Branches at this level per parent branch.
    pub count: u32,
    /// Length of a branch as a fraction of its parent's (the trunk: of the tree height).
    pub length: f32,
    /// Random variation of that length, 0..=1.
    pub length_variation: f32,
    /// Radius of a branch as a fraction of its parent's radius at the fork.
    pub radius_ratio: f32,
    /// How fast the branch thins toward its tip, 0 (none) to 1 (a point).
    pub taper: f32,
    /// Angle between a branch and its parent, in degrees.
    pub down_angle_deg: f32,
    /// Random variation of that angle, in degrees.
    pub down_variation_deg: f32,
    /// Angle around the parent between successive branches, in degrees.
    pub rotate_deg: f32,
    /// Random variation of that angle, in degrees.
    pub rotate_variation_deg: f32,
    /// How much the branch bends along its length, in degrees in total.
    pub curve_deg: f32,
    /// Segments (nodes) along a branch.
    pub segments: u32,
    /// Where along the parent the first branch starts, 0..1.
    pub start: f32,
    /// How much gravity bends the branch down (negative lifts it up), -1..=1.
    pub gravity: f32,
}

/// A whole tree: height, trunk radius, 1 to 4 levels of branching and the leaves.
#[derive(Debug, Clone, PartialEq)]
pub struct Tree {
    /// Seed.
    pub seed: u32,
    /// Height in metres.
    pub height: f32,
    /// Trunk radius at the base in metres.
    pub trunk_radius: f32,
    /// The levels of branching, 1 to 4.
    pub levels: Vec<Level>,
    /// Leaf length in metres.
    pub leaf_size: f32,
    /// Leaves per twig tip.
    pub leaves_per_tip: u32,
    /// Leaves grow on nodes of at least this depth.
    pub leaf_depth_from: u8,
}

/// Grows a tree.
///
/// # Errors
/// `Cancelled`, `TooLarge`, `BadParameter` (levels outside 1..=4, non-positive size).
#[expect(
    clippy::too_many_lines,
    reason = "tree growth keeps validation, ordered branch creation and leaf attachment in one transaction"
)]
pub fn grow_tree(params: &Tree, cancel: &dyn Fn() -> bool) -> Result<Skeleton, FloraError> {
    validate(params)?;
    let mut skeleton = Skeleton::default();
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
    let trunk_segments = params.levels[0].segments;
    let mut trunk = vec![root];
    let mut direction = [0.0, 1.0, 0.0];
    let mut position = [0.0; 3];
    let mut random = SplitMix64::new(params.seed);
    for index in 1..=trunk_segments {
        if cancel() {
            return Err(FloraError::Cancelled);
        }
        let level = params.levels[0];
        let axis = math::perpendicular(direction);
        let bend = level.curve_deg.to_radians() / math::count_f32(trunk_segments);
        direction = math::rotate(direction, axis, bend);
        direction = math::unit(math::add(direction, [0.0, -level.gravity * 0.02, 0.0]));
        position = math::add(
            position,
            math::scale(direction, params.height / math::count_f32(trunk_segments)),
        );
        let fraction = math::count_f32(index) / math::count_f32(trunk_segments);
        let radius = params.trunk_radius * (1.0 - 0.35 * fraction);
        let node = add_node(
            &mut skeleton,
            position,
            radius,
            *trunk.last().unwrap_or(&0),
            0,
            0,
            fraction,
            fraction,
            params.seed,
        );
        trunk.push(node);
        let _ = random.next();
    }
    let mut parents = vec![(trunk, params.height, direction)];
    let mut next_branch = 1_u32;
    for (depth_index, level) in params.levels.iter().enumerate().skip(1) {
        let depth = u8::try_from(depth_index).unwrap_or(u8::MAX);
        let mut children = Vec::new();
        for (parent_nodes, parent_length, _parent_direction) in parents {
            if cancel() {
                return Err(FloraError::Cancelled);
            }
            if parent_nodes.len() < 2 {
                continue;
            }
            for child_index in 0..level.count {
                if skeleton.node_count() + (level.segments as usize * level.count as usize)
                    > 200_000
                {
                    return Err(too_large(
                        skeleton.node_count(),
                        "lower branch count or segments",
                    ));
                }
                let spacing =
                    (math::count_f32(child_index) + 1.0) / (math::count_f32(level.count) + 1.0);
                let ratio = level.start + spacing * (1.0 - level.start);
                let sample = math::bounded_index(
                    ratio * math::length_f32(parent_nodes.len() - 1),
                    parent_nodes.len() - 1,
                );
                let fork = parent_nodes[sample.min(parent_nodes.len() - 1)];
                let parent_before = parent_nodes[sample.saturating_sub(1)];
                let parent_after = parent_nodes[(sample + 1).min(parent_nodes.len() - 1)];
                let tangent = math::unit(math::sub(
                    skeleton.position[parent_after as usize],
                    skeleton.position[parent_before as usize],
                ));
                let around = (math::count_f32(child_index) * level.rotate_deg
                    + random.signed() * level.rotate_variation_deg)
                    .to_radians();
                let radial = math::rotate(math::perpendicular(tangent), tangent, around);
                let down = (level.down_angle_deg + random.signed() * level.down_variation_deg)
                    .to_radians();
                let mut branch_direction = math::unit(math::rotate(tangent, radial, down));
                branch_direction = math::unit(math::add(
                    branch_direction,
                    [0.0, -level.gravity * 0.2, 0.0],
                ));
                let branch_length =
                    parent_length * level.length * (1.0 + random.signed() * level.length_variation);
                let phase = wind::branch_phase(params.seed, next_branch);
                let branch_nodes = grow_branch(
                    params,
                    &mut skeleton,
                    fork,
                    branch_direction,
                    branch_length,
                    depth,
                    next_branch,
                    phase,
                    *level,
                    cancel,
                )?;
                children.push((branch_nodes, branch_length, branch_direction));
                next_branch = next_branch.saturating_add(1);
            }
        }
        parents = children;
    }
    normalize_height(&mut skeleton, params.height);
    add_leaves(params, &mut skeleton);
    Ok(skeleton)
}

fn normalize_height(skeleton: &mut Skeleton, height: f32) {
    let current_height = skeleton.height();
    if current_height <= 0.0 {
        return;
    }
    let scale = height / current_height;
    for position in &mut skeleton.position {
        position[1] *= scale;
    }
    for leaf in &mut skeleton.leaves {
        leaf.position[1] *= scale;
    }
}

fn validate(params: &Tree) -> Result<(), FloraError> {
    if !(1..=4).contains(&params.levels.len()) {
        return Err(bad("levels", "use one to four levels"));
    }
    if !params.height.is_finite() || params.height <= 0.0 {
        return Err(bad("height", "must be finite and positive"));
    }
    if !params.trunk_radius.is_finite() || params.trunk_radius <= 0.0 {
        return Err(bad("trunk_radius", "must be finite and positive"));
    }
    for (index, level) in params.levels.iter().enumerate() {
        if level.segments == 0
            || (index > 0 && level.count == 0)
            || !level.length.is_finite()
            || level.length <= 0.0
        {
            return Err(bad(
                "levels",
                "segments, branch counts and lengths must be positive",
            ));
        }
        if !(0.0..=1.0).contains(&level.length_variation)
            || !(0.0..=1.0).contains(&level.taper)
            || !(0.0..=1.0).contains(&level.start)
            || !level.radius_ratio.is_finite()
            || !(0.0..=1.0).contains(&level.radius_ratio)
            || !level.gravity.is_finite()
            || !(-1.0..=1.0).contains(&level.gravity)
        {
            return Err(bad(
                "levels",
                "variation, taper, start, radius ratio or gravity is out of range",
            ));
        }
    }
    Ok(())
}

fn grow_branch(
    params: &Tree,
    skeleton: &mut Skeleton,
    fork: u32,
    initial: [f32; 3],
    length: f32,
    depth: u8,
    branch: u32,
    phase: f32,
    level: Level,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<u32>, FloraError> {
    let mut nodes = Vec::with_capacity(level.segments as usize);
    let mut position = skeleton.position[fork as usize];
    let mut direction = initial;
    for segment in 1..=level.segments {
        if cancel() {
            return Err(FloraError::Cancelled);
        }
        let axis = math::perpendicular(direction);
        direction = math::rotate(
            direction,
            axis,
            level.curve_deg.to_radians() / math::count_f32(level.segments),
        );
        direction = math::unit(math::add(direction, [0.0, -level.gravity * 0.08, 0.0]));
        position = math::add(
            position,
            math::scale(direction, length / math::count_f32(level.segments)),
        );
        let fraction = math::count_f32(segment) / math::count_f32(level.segments);
        let fork_radius = skeleton.radius[fork as usize] * level.radius_ratio;
        let radius = fork_radius * (1.0 - level.taper * fraction).max(0.03);
        let id = push_node(
            skeleton,
            position,
            radius,
            if segment == 1 {
                fork
            } else {
                *nodes.last().unwrap_or(&fork)
            },
            depth,
            branch,
            (position[1] / params.height).clamp(0.0, 1.0),
            fraction,
            phase,
        );
        nodes.push(id);
    }
    Ok(nodes)
}

fn add_node(
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
    push_node(
        skeleton,
        position,
        radius,
        parent,
        depth,
        branch,
        height,
        fraction,
        wind::branch_phase(seed, branch),
    )
}

fn push_node(
    skeleton: &mut Skeleton,
    position: [f32; 3],
    radius: f32,
    parent: u32,
    depth: u8,
    branch: u32,
    height: f32,
    fraction: f32,
    phase: f32,
) -> u32 {
    let id = u32::try_from(skeleton.node_count()).unwrap_or(u32::MAX);
    skeleton.position.push(position);
    skeleton.radius.push(radius.max(1.0e-5));
    skeleton.parent.push(parent);
    skeleton.depth.push(depth);
    wind::append(skeleton, branch, depth, height, fraction, phase);
    id
}

fn add_leaves(params: &Tree, skeleton: &mut Skeleton) {
    if params.leaf_size <= 0.0 || params.leaves_per_tip == 0 {
        return;
    }
    for node in 1..skeleton.node_count() {
        if skeleton.depth[node] < params.leaf_depth_from {
            continue;
        }
        let child = skeleton.position[node];
        let parent = skeleton.position[skeleton.parent[node] as usize];
        let direction = math::unit(math::sub(child, parent));
        for leaf_index in 0..params.leaves_per_tip {
            let angle = math::count_f32(leaf_index) * 2.399_963_1;
            let side = math::rotate(math::perpendicular(direction), direction, angle);
            let leaf_direction = math::unit(math::add(math::scale(direction, 0.5), side));
            let spread = math::scale(side, params.leaf_size * 3.5);
            let mut position = math::add(child, spread);
            if params.leaf_depth_from >= 2 {
                position[1] = position[1].max(params.height * 0.25);
            }
            skeleton.leaves.push(super::Leaf {
                position,
                direction: leaf_direction,
                size: params.leaf_size,
                node: u32::try_from(node).unwrap_or(u32::MAX),
            });
        }
    }
}

fn bad(name: &'static str, why: &str) -> FloraError {
    FloraError::BadParameter {
        name,
        why: why.to_owned(),
    }
}

fn too_large(nodes: usize, hint: &str) -> FloraError {
    FloraError::TooLarge {
        nodes,
        max: 200_000,
        hint: hint.to_owned(),
    }
}
