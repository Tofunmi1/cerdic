//! Grass and small plants.
use super::{FloraError, Leaf, Skeleton, math, random::SplitMix64, wind};

/// A tuft of curved blades from one root.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tuft {
    /// Seed.
    pub seed: u32,
    /// Number of blades (1..=200).
    pub blades: u32,
    /// Smallest and largest blade height in metres.
    pub height: [f32; 2],
    /// Blade width at the base in metres (the node radius is half of it).
    pub width: f32,
    /// How far a blade bends over, in degrees in total.
    pub bend_deg: f32,
    /// Radius of the patch the blades start in, in metres.
    pub spread: f32,
}

/// Grows a tuft: one root node, each blade a chain of nodes that is its own branch.
///
/// # Errors
/// `Cancelled`, `BadParameter`.
pub fn grow_tuft(params: &Tuft, cancel: &dyn Fn() -> bool) -> Result<Skeleton, FloraError> {
    validate(params)?;
    let mut skeleton = Skeleton::default();
    skeleton.position.push([0.0; 3]);
    skeleton.radius.push((params.width * 0.5).max(1.0e-5));
    skeleton.parent.push(u32::MAX);
    skeleton.depth.push(0);
    wind::append(
        &mut skeleton,
        0,
        0,
        0.0,
        0.0,
        wind::branch_phase(params.seed, 0),
    );
    let mut random = SplitMix64::new(params.seed);
    let segments = 4_u32;
    for blade in 0..params.blades {
        if cancel() {
            return Err(FloraError::Cancelled);
        }
        let angle = (math::count_f32(blade) / math::count_f32(params.blades) * 360.0
            + random.signed() * 12.0)
            .to_radians();
        let (sine, cosine) = math::sin_cos(angle);
        let base = [
            cosine * random.unit() * params.spread,
            0.0,
            sine * random.unit() * params.spread,
        ];
        let height = params.height[0] + random.unit() * (params.height[1] - params.height[0]);
        let branch = blade + 1;
        let phase = wind::branch_phase(params.seed, branch);
        let mut previous = 0_u32;
        let mut position = base;
        for segment in 1..=segments {
            let fraction = math::count_f32(segment) / math::count_f32(segments);
            let bend = params.bend_deg.to_radians() * fraction;
            let (bend_sine, bend_cosine) = math::sin_cos(bend);
            let horizontal = [sine * bend_sine, 0.0, -cosine * bend_sine];
            let direction = math::unit([horizontal[0], bend_cosine, horizontal[2]]);
            position = math::add(
                position,
                math::scale(direction, height / math::count_f32(segments)),
            );
            let radius = (params.width * 0.5 * (1.0 - 0.8 * fraction)).max(1.0e-5);
            let node = u32::try_from(skeleton.node_count()).unwrap_or(u32::MAX);
            skeleton.position.push(position);
            skeleton.radius.push(radius);
            skeleton
                .parent
                .push(if segment == 1 { 0 } else { previous });
            skeleton.depth.push(1);
            wind::append(
                &mut skeleton,
                branch,
                1,
                (position[1] / params.height[1]).clamp(0.0, 1.0),
                fraction,
                phase,
            );
            previous = node;
        }
        let tip = skeleton.position[previous as usize];
        let direction = math::unit(math::sub(tip, base));
        skeleton.leaves.push(Leaf {
            position: tip,
            direction,
            size: params.width,
            node: previous,
        });
    }
    if skeleton.node_count() > 200_000 {
        return Err(FloraError::TooLarge {
            nodes: skeleton.node_count(),
            max: 200_000,
            hint: "lower blades".to_owned(),
        });
    }
    Ok(skeleton)
}

fn validate(params: &Tuft) -> Result<(), FloraError> {
    if params.blades == 0 || params.blades > 200 {
        return Err(FloraError::BadParameter {
            name: "blades",
            why: "use 1 to 200 blades".to_owned(),
        });
    }
    if !params.height[0].is_finite()
        || !params.height[1].is_finite()
        || params.height[0] <= 0.0
        || params.height[1] < params.height[0]
    {
        return Err(FloraError::BadParameter {
            name: "height",
            why: "use finite positive heights in ascending order".to_owned(),
        });
    }
    if !params.width.is_finite()
        || params.width <= 0.0
        || !params.spread.is_finite()
        || params.spread < 0.0
        || !params.bend_deg.is_finite()
    {
        return Err(FloraError::BadParameter {
            name: "width",
            why: "width and spread must be finite and positive, and bend finite".to_owned(),
        });
    }
    Ok(())
}
