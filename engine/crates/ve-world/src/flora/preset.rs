//! Named plant presets a model can pick.
use super::{Colonize, Crown, FloraError, Tree};

/// A complete plant recipe.
#[derive(Debug, Clone, PartialEq)]
pub enum Preset {
    /// A parametric tree.
    Tree(Tree),
    /// A space-colonization crown.
    Colonize(Colonize),
}

/// The names [`preset`] accepts.
pub const NAMES: [&str; 6] = ["oak", "pine", "birch", "willow", "shrub", "palm"];

/// The recipe for a named plant at a height in metres: `oak`, `pine`, `birch`, `willow`, `shrub` or `palm`.
///
/// # Errors
/// `BadParameter` for an unknown name (the message lists the valid ones) or a non-positive height.
pub fn preset(name: &str, height: f32, seed: u32) -> Result<Preset, FloraError> {
    if !height.is_finite() || height <= 0.0 {
        return Err(FloraError::BadParameter {
            name: "height",
            why: "must be finite and positive".to_owned(),
        });
    }
    named_recipe(name, height, seed).ok_or_else(|| FloraError::BadParameter {
        name: "preset",
        why: format!("unknown plant; valid names: {}", NAMES.join(", ")),
    })
}

fn named_recipe(name: &str, height: f32, seed: u32) -> Option<Preset> {
    match name {
        "oak" => Some(oak(height, seed)),
        "pine" => Some(pine(height, seed)),
        "birch" => Some(birch(height, seed)),
        "willow" => Some(willow(height, seed)),
        "shrub" => Some(shrub(height, seed)),
        "palm" => Some(Preset::Tree(tree(
            seed,
            height,
            0.06,
            &[8],
            &[0.8],
            &[28.0],
            12,
            0.55,
            1,
            1,
        ))),
        _ => None,
    }
}

fn oak(height: f32, seed: u32) -> Preset {
    colonize(
        seed,
        height,
        0.045,
        0.30 * height,
        Crown::Ellipsoid {
            center: [0.0, 0.65 * height, 0.0],
            radii: [0.62 * height, 0.35 * height, 0.62 * height],
        },
        2_200,
        1.8 * height / 10.0,
        0.38 * height / 10.0,
        0.22 * height / 10.0,
        360,
    )
}

fn pine(height: f32, seed: u32) -> Preset {
    colonize(
        seed,
        height,
        0.035,
        0.20 * height,
        Crown::Cone {
            base: [0.0, 0.20 * height, 0.0],
            height: 0.80 * height,
            radius: 0.31 * height,
        },
        2_200,
        1.45 * height / 10.0,
        0.32 * height / 10.0,
        0.20 * height / 10.0,
        320,
    )
}

fn birch(height: f32, seed: u32) -> Preset {
    colonize(
        seed,
        height,
        0.025,
        0.25 * height,
        Crown::Ellipsoid {
            center: [0.0, 0.625 * height, 0.0],
            radii: [0.28 * height, 0.375 * height, 0.28 * height],
        },
        2_000,
        1.4 * height / 10.0,
        0.30 * height / 10.0,
        0.19 * height / 10.0,
        320,
    )
}

fn willow(height: f32, seed: u32) -> Preset {
    colonize(
        seed,
        height,
        0.04,
        0.23 * height,
        Crown::Ellipsoid {
            center: [0.0, 0.59 * height, 0.0],
            radii: [0.78 * height, 0.36 * height, 0.78 * height],
        },
        2_400,
        1.9 * height / 10.0,
        0.40 * height / 10.0,
        0.21 * height / 10.0,
        380,
    )
}

fn shrub(height: f32, seed: u32) -> Preset {
    colonize(
        seed,
        height,
        0.012,
        0.035 * height,
        Crown::Sphere {
            center: [0.0, 0.55 * height, 0.0],
            radius: 0.55 * height,
        },
        3_500,
        2.0 * height / 10.0,
        0.35 * height / 10.0,
        0.30 * height / 10.0,
        450,
    )
}

fn colonize(
    seed: u32,
    height: f32,
    trunk_radius: f32,
    trunk_height: f32,
    crown: Crown,
    points: u32,
    attraction_radius: f32,
    kill_distance: f32,
    step: f32,
    max_iterations: u32,
) -> Preset {
    Preset::Colonize(Colonize {
        seed,
        trunk_height,
        trunk_radius: trunk_radius * height,
        crown,
        points,
        attraction_radius,
        kill_distance,
        step,
        radius_exponent: 2.0,
        max_iterations,
    })
}

fn tree(
    seed: u32,
    height: f32,
    radius: f32,
    counts: &[u32],
    lengths: &[f32],
    down: &[f32],
    segments: u32,
    leaf_size: f32,
    leaves_per_tip: u32,
    leaf_depth_from: u8,
) -> Tree {
    let trunk = super::Level {
        count: 1,
        length: 1.0,
        length_variation: 0.0,
        radius_ratio: 0.72,
        taper: 0.35,
        down_angle_deg: 0.0,
        down_variation_deg: 0.0,
        rotate_deg: 137.5,
        rotate_variation_deg: 0.0,
        curve_deg: 8.0,
        segments,
        start: 0.0,
        gravity: 0.0,
    };
    let mut levels = vec![trunk];
    for (index, count) in counts.iter().enumerate() {
        let droop = down[index] > 65.0;
        levels.push(super::Level {
            count: *count,
            length: lengths[index],
            length_variation: 0.12,
            radius_ratio: if index == 0 { 0.48 } else { 0.42 },
            taper: 0.82,
            down_angle_deg: down[index],
            down_variation_deg: 8.0,
            rotate_deg: 137.5,
            rotate_variation_deg: 12.0,
            curve_deg: if droop { 24.0 } else { 14.0 },
            segments: (segments / 2).max(3),
            start: if counts[0] == 12 {
                0.30
            } else if counts[0] == 3 && leaf_depth_from == 1 {
                0.0
            } else if leaf_depth_from >= 2 {
                0.50
            } else {
                0.28
            },
            gravity: if droop { 0.55 } else { 0.12 },
        });
    }
    Tree {
        seed,
        height,
        // Trunk radius grows with height (an 11 m oak is about 0.27 m across the base; the sqrt this replaced gave 0.15 m, a sapling).
        trunk_radius: radius * height * 0.55,
        levels,
        leaf_size: leaf_size * height / 8.0,
        leaves_per_tip,
        leaf_depth_from,
    }
}
