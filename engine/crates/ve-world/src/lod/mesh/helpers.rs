//! Geometry helpers shared by plant meshing.
use super::super::{
    LeafCards, LodError, MaterialRange, Meshing, PlantMesh, Surface, Vertex, WindData,
};
use crate::flora::Skeleton;
pub(super) mod math;

const MAX_VERTICES: usize = 400_000;

pub(super) fn validate(params: &Meshing) -> Result<(), LodError> {
    if !(3..=16).contains(&params.radial_segments) {
        return Err(bad("radial_segments", "must be in 3..=16"));
    }
    if !params.max_segment_length.is_finite() || params.max_segment_length <= 0.0 {
        return Err(bad("max_segment_length", "must be finite and positive"));
    }
    if params
        .bark_uv_scale
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(bad(
            "bark_uv_scale",
            "both scales must be finite and positive",
        ));
    }
    if !params.fork_blend.is_finite() || !(0.0..=1.0).contains(&params.fork_blend) {
        return Err(bad("fork_blend", "must be in 0..=1"));
    }
    if params.smooth > 6 {
        return Err(bad("smooth", "must be in 0..=6"));
    }
    if !params.flare.is_finite() || !(0.0..=3.0).contains(&params.flare) {
        return Err(bad("flare", "must be in 0..=3"));
    }
    let card_values = match params.leaf_cards {
        LeafCards::Quads { aspect, bend_deg } => [aspect, bend_deg],
        LeafCards::Crossed { aspect } => [aspect, 0.0],
        LeafCards::Clumps {
            leaves,
            spread,
            aspect,
        } => {
            if !(2..=8).contains(&leaves) || !spread.is_finite() {
                return Err(bad(
                    "leaf_cards",
                    "clump leaves must be 2..=8 and spread finite",
                ));
            }
            [aspect, spread]
        }
    };
    if !card_values[0].is_finite() || card_values[0] <= 0.0 || !card_values[1].is_finite() {
        return Err(bad(
            "leaf_cards",
            "aspect must be positive and angles finite",
        ));
    }
    Ok(())
}

pub(super) fn add_tube(
    mesh: &mut PlantMesh,
    skeleton: &Skeleton,
    line: &[u32],
    params: &Meshing,
) -> Result<(), LodError> {
    if line.len() < 2 {
        return Ok(());
    }
    let mut samples = tube_samples(skeleton, line, params);
    if samples.len() < 2 {
        return Ok(());
    }
    if line.len() > 1 {
        let fork = line[0] as usize;
        let child = line[1] as usize;
        let is_child_branch = skeleton.branch.get(fork) != skeleton.branch.get(child);
        if is_child_branch {
            let fork_radius = samples[0].radius;
            let child_radius = samples[1].radius;
            samples[0].radius = child_radius + (fork_radius - child_radius) * params.fork_blend;
        }
    }
    check_size(
        mesh.vertices.len().saturating_add(
            samples
                .len()
                .saturating_mul(params.radial_segments as usize + 1),
        ),
    )?;
    let sides = side_count(
        params.radial_segments,
        samples
            .iter()
            .map(|sample| sample.radius)
            .fold(0.0_f32, f32::max),
    ) as usize;
    let mut previous_tangent = math::unit(math::sub(samples[1].position, samples[0].position));
    let mut basis = math::perpendicular(previous_tangent);
    let mut arc = 0.0;
    let start = mesh.vertices.len();
    for (sample_index, sample) in samples.iter().enumerate() {
        let tangent = if sample_index + 1 < samples.len() {
            math::unit(math::sub(
                samples[sample_index + 1].position,
                sample.position,
            ))
        } else {
            previous_tangent
        };
        if sample_index > 0 {
            arc += math::length(math::sub(
                sample.position,
                samples[sample_index - 1].position,
            ));
            let axis = cross(previous_tangent, tangent);
            let axis_length = math::length(axis);
            if axis_length > 1.0e-6 {
                let angle = axis_length.atan2(math::dot(previous_tangent, tangent));
                basis = math::rotate(basis, axis, angle);
            }
        }
        append_ring(
            mesh,
            skeleton,
            sample,
            tangent,
            basis,
            sides,
            arc,
            params.bark_uv_scale[1],
        );
        if sample_index > 0 {
            add_ring_indices(mesh, start, sample_index - 1, sample_index, sides);
        }
        previous_tangent = tangent;
    }
    Ok(())
}

#[expect(
    clippy::cast_precision_loss,
    reason = "radial segment counts are bounded to sixteen"
)]
fn append_ring(
    mesh: &mut PlantMesh,
    skeleton: &Skeleton,
    sample: &Sample,
    tangent: [f32; 3],
    basis: [f32; 3],
    sides: usize,
    arc: f32,
    uv_scale: f32,
) {
    let binormal = math::unit(cross(tangent, basis));
    // one vertex more than there are sides: the last repeats the first with u = 1, so the bark texture does not smear back across the seam
    for side in 0..=sides {
        let side_fraction = side as f32 / sides as f32;
        let (sine, cosine) = math::sin_cos(core::f32::consts::TAU * side_fraction);
        let normal = math::unit(math::add(
            math::scale(basis, cosine),
            math::scale(binormal, sine),
        ));
        // root flare: five lobes that fade out with height above the soil
        let swell = 1.0 + sample.flare * (0.55 + 0.45 * math::sin_cos(5.0 * core::f32::consts::TAU * side_fraction + sample.phase).1);
        let position = math::add(
            sample.position,
            math::scale(normal, (sample.radius * swell).max(0.0001)),
        );
        mesh.vertices.push(Vertex {
            position,
            normal,
            uv: [side_fraction, arc / uv_scale],
            tangent: [binormal[0], binormal[1], binormal[2], 1.0],
            wind: bark_wind(skeleton, sample.node, position[1]),
        });
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "mesh vertex count is capped at 400,000"
)]
fn add_ring_indices(
    mesh: &mut PlantMesh,
    start: usize,
    prev_ring: usize,
    next_ring: usize,
    ring_sides: usize,
) {
    let stride = ring_sides + 1;
    let old_offset = start + prev_ring * stride;
    let new_offset = start + next_ring * stride;
    for side in 0..ring_sides {
        let next = side + 1;
        let a = (old_offset + side) as u32;
        let b = (old_offset + next) as u32;
        let c = (new_offset + next) as u32;
        let d = (new_offset + side) as u32;
        mesh.indices.extend_from_slice(&[a, b, c, a, c, d]);
    }
}

#[derive(Clone, Copy)]
struct Sample {
    position: [f32; 3],
    radius: f32,
    node: usize,
    /// Root flare strength at this sample (0 away from the soil).
    flare: f32,
    /// Where the five root lobes start turning, so no two trees share a pattern.
    phase: f32,
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "skeleton nodes are bounded by the flora generator"
)]
#[expect(
    clippy::cast_sign_loss,
    reason = "segment lengths and max_length are positive"
)]
#[expect(
    clippy::cast_precision_loss,
    reason = "subdivision counts are bounded by the skeleton node limit"
)]
fn tube_samples(skeleton: &Skeleton, line: &[u32], params: &Meshing) -> Vec<Sample> {
    let position = |node: u32| skeleton.position[node as usize];
    // the trunk is the branch that starts at the root; only it swells into roots, over a few trunk radii of height
    let trunk = line.first() == Some(&0) && skeleton.depth.first() == Some(&0);
    let base_height = position(0)[1];
    let flare_height = (1.2 * skeleton.radius[0]).max(0.12);
    let flare_at = |y: f32| {
        if trunk && params.flare > 0.0 {
            params.flare * (-(y - base_height).max(0.0) / flare_height).exp()
        } else {
            0.0
        }
    };
    let phase = skeleton.phase.first().copied().unwrap_or(0.0) * core::f32::consts::TAU;
    // only limbs are worth curving: a twig is a few pixels wide, and there are tens of thousands of them
    // (the first node of a branch's line is where it forks off its parent, so it carries the parent's radius and is left out)
    let thickest = line.iter().skip(1).map(|&node| skeleton.radius[node as usize]).fold(0.0_f32, f32::max);
    let smooth = if thickest >= 0.05 { params.smooth } else { 0 };
    let mut samples = Vec::new();
    for (index, pair) in line.windows(2).enumerate() {
        let from = pair[0] as usize;
        let to = pair[1] as usize;
        let (a, b) = (skeleton.position[from], skeleton.position[to]);
        // the neighbours on either side, for a curve that passes through the nodes with a continuous direction (the ends repeat themselves)
        let before = if index == 0 { math::sub(math::scale(a, 2.0), b) } else { position(line[index - 1]) };
        let after = line.get(index + 2).map_or_else(|| math::sub(math::scale(b, 2.0), a), |&node| position(node));
        let length = math::length(math::sub(b, a));
        let mut longest = params.max_segment_length;
        if trunk && params.flare > 0.0 && a[1] - base_height < 5.0 * flare_height {
            // finer rings where the roots swell
            longest = longest.min(flare_height * 0.4);
        }
        let mut divisions = (length / longest).ceil().max(1.0) as usize;
        if smooth > 0 {
            divisions = divisions.max(smooth as usize + 1);
        }
        for part in 0..divisions {
            let t = part as f32 / divisions as f32;
            let point = if smooth > 0 { catmull_rom(before, a, b, after, t) } else { math::add(a, math::scale(math::sub(b, a), t)) };
            samples.push(Sample {
                position: point,
                radius: skeleton.radius[from] + (skeleton.radius[to] - skeleton.radius[from]) * t,
                node: if t < 0.5 { from } else { to },
                flare: flare_at(point[1]),
                phase,
            });
        }
    }
    if let Some(&last) = line.last() {
        let node = last as usize;
        samples.push(Sample {
            position: skeleton.position[node],
            radius: skeleton.radius[node],
            node,
            flare: flare_at(skeleton.position[node][1]),
            phase,
        });
    }
    samples
}

/// The point at `t` (0 at `p1`, 1 at `p2`) of the Catmull-Rom curve through four consecutive points.
fn catmull_rom(p0: [f32; 3], p1: [f32; 3], p2: [f32; 3], p3: [f32; 3], t: f32) -> [f32; 3] {
    let (t2, t3) = (t * t, t * t * t);
    let mut out = [0.0; 3];
    for axis in 0..3 {
        out[axis] = 0.5
            * (2.0 * p1[axis]
                + (p2[axis] - p0[axis]) * t
                + (2.0 * p0[axis] - 5.0 * p1[axis] + 4.0 * p2[axis] - p3[axis]) * t2
                + (3.0 * p1[axis] - p0[axis] - 3.0 * p2[axis] + p3[axis]) * t3);
    }
    out
}

fn side_count(maximum: u32, radius: f32) -> u32 {
    if radius < 0.015 {
        3
    } else if radius < 0.04 {
        maximum.clamp(3, 5)
    } else if radius < 0.1 {
        maximum.clamp(3, 8)
    } else {
        maximum
    }
}

fn bark_wind(skeleton: &Skeleton, node: usize, height: f32) -> WindData {
    let plant_height = skeleton.height().max(0.001);
    let phase = skeleton
        .phase
        .get(node)
        .copied()
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let depth = skeleton.depth.get(node).copied().unwrap_or(0);
    let wind = skeleton.wind.get(node).copied().unwrap_or(0.0);
    let stiffness = skeleton.stiffness.get(node).copied().unwrap_or(1.0);
    WindData {
        pivot: skeleton
            .position
            .get(skeleton.parent.get(node).copied().unwrap_or(u32::MAX) as usize)
            .copied()
            .unwrap_or([0.0; 3]),
        phase,
        trunk: (height / plant_height).clamp(0.0, 1.0),
        branch: if depth == 0 {
            0.0
        } else {
            (wind * (1.0 - stiffness)).clamp(0.0, 1.0)
        },
        leaf: 0.0,
        edge: 0.0,
    }
}

pub(super) fn check_size(vertices: usize) -> Result<(), LodError> {
    if vertices > MAX_VERTICES {
        return Err(LodError::TooLarge {
            vertices,
            max: MAX_VERTICES,
        });
    }
    Ok(())
}

pub(super) fn range(kind: Surface, start: usize, end: usize) -> MaterialRange {
    MaterialRange {
        kind,
        first_index: u32::try_from(start).unwrap_or(u32::MAX),
        index_count: u32::try_from(end - start).unwrap_or(u32::MAX),
    }
}

fn bad(name: &'static str, why: &str) -> LodError {
    LodError::BadParameter {
        name,
        why: why.to_owned(),
    }
}

pub(super) fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
