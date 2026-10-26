//! Carving a river channel into local terrain.
use super::geometry::{nearest, overlaps_grid, path_samples};
use super::{Ground, Path, RiverError};

/// The cross-section of a river.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RiverProfile {
    /// Width of the channel floor and water, in metres.
    pub width: f32,
    /// Depth of the bed below the surrounding ground, in metres.
    pub depth: f32,
    /// Angle of the banks from flat, in degrees, 5..=85.
    pub bank_slope_deg: f32,
    /// Extra width on each side over which the banks blend into the ground, in metres.
    pub bank_width: f32,
    /// How much the valley floor is smoothed across the channel, 0..=1.
    pub meander_flatten: f32,
    /// How far the water surface lies below the surrounding bank top, in metres.
    pub water_level_offset: f32,
}

/// What a river carve produces, one value per cell of the ground.
#[derive(Debug, Clone, PartialEq)]
pub struct RiverOut {
    /// The water surface height; where there is no water, the ground height.
    pub water_height: Vec<f32>,
    /// Unit direction of flow in world x, z; zero outside the channel.
    pub flow_dir: Vec<[f32; 2]>,
    /// Speed of flow in metres per second; zero outside the channel.
    pub flow_speed: Vec<f32>,
    /// 0..=1, wet banks and sand.
    pub bank_mask: Vec<f32>,
    /// 0..=1, the channel itself.
    pub channel_mask: Vec<f32>,
}

/// Carves a channel along `path` into the ground. The bed is the surrounding ground minus `depth`, shaped by the profile, so a cell's
/// result depends only on ground within `width / 2 + bank_width` of it and tiles of one world agree.
///
/// # Errors
/// `Cancelled`, `BadShape`, `BadPath` (fewer than 2 distinct points, or none inside the grid), `BadParameter`.
pub fn carve_river(
    ground: &mut Ground<'_>,
    path: &Path,
    profile: &RiverProfile,
    cancel: &dyn Fn() -> bool,
) -> Result<RiverOut, RiverError> {
    ground.check()?;
    validate_profile(profile)?;
    if cancel() {
        return Err(RiverError::Cancelled);
    }
    let samples = path_samples(path, ground.cell * 0.5)?;
    if !overlaps_grid(&samples, ground) {
        return Err(RiverError::BadPath {
            why: "the path does not cross the grid".into(),
        });
    }
    let original = ground.heights.to_vec();
    let count = original.len();
    let mut output = RiverOut {
        water_height: original.clone(),
        flow_dir: vec![[0.0, 0.0]; count],
        flow_speed: vec![0.0; count],
        bank_mask: vec![0.0; count],
        channel_mask: vec![0.0; count],
    };
    let outer = profile.width * 0.5 + profile.bank_width;
    let bank_tangent = tangent(profile.bank_slope_deg);
    for z in 0..ground.depth {
        if cancel() {
            return Err(RiverError::Cancelled);
        }
        for x in 0..ground.width {
            let index = z * ground.width + x;
            let world = [
                ground.origin[0] + crate::num::cell_f32(x) * ground.cell,
                ground.origin[1] + crate::num::cell_f32(z) * ground.cell,
            ];
            let (distance, direction, along) = nearest(&samples, world);
            if distance > outer {
                continue;
            }
            let side = [-direction[1], direction[0]];
            let bank_a = sample_height(
                &original,
                ground,
                [world[0] + side[0] * outer, world[1] + side[1] * outer],
            );
            let bank_b = sample_height(
                &original,
                ground,
                [world[0] - side[0] * outer, world[1] - side[1] * outer],
            );
            let local_bank = (bank_a + bank_b) * 0.5;
            let bank_top = original[index] * (1.0 - profile.meander_flatten)
                + local_bank * profile.meander_flatten;
            let flat_radius = profile.width * 0.5;
            let rise_distance = (profile.depth / bank_tangent)
                .min(profile.bank_width)
                .max(ground.cell);
            let channel = 1.0 - smoothstep(flat_radius, flat_radius + rise_distance, distance);
            let bank = 1.0 - smoothstep(flat_radius, outer, distance);
            let vertical_rise =
                ((distance - flat_radius).max(0.0) * bank_tangent).min(profile.depth);
            let target = bank_top - profile.depth + vertical_rise;
            let blend = if distance <= flat_radius { 1.0 } else { bank };
            let carved = original[index] + (target - original[index]) * blend;
            ground.heights[index] = carved;
            output.channel_mask[index] = channel;
            output.bank_mask[index] = (bank - channel).max(0.0);
            if distance <= flat_radius {
                output.water_height[index] = bank_top - profile.water_level_offset;
                output.flow_dir[index] = direction;
                output.flow_speed[index] =
                    (profile.depth * 0.2 + slope_speed(&samples, along)).max(0.01);
            }
        }
    }
    Ok(output)
}

fn validate_profile(profile: &RiverProfile) -> Result<(), RiverError> {
    for (name, value) in [
        ("width", profile.width),
        ("depth", profile.depth),
        ("bank_width", profile.bank_width),
        ("water_level_offset", profile.water_level_offset),
    ] {
        if !value.is_finite() || value < 0.0 || (name == "width" && value <= 0.0) {
            return Err(RiverError::BadParameter {
                name,
                why: "the value must be finite and non-negative (width must be positive)".into(),
            });
        }
    }
    if !(0.0..=1.0).contains(&profile.meander_flatten) || !profile.meander_flatten.is_finite() {
        return Err(RiverError::BadParameter {
            name: "meander_flatten",
            why: "the value must be in 0..=1".into(),
        });
    }
    if !(5.0..=85.0).contains(&profile.bank_slope_deg) {
        return Err(RiverError::BadParameter {
            name: "bank_slope_deg",
            why: "the angle must be in 5..=85 degrees".into(),
        });
    }
    Ok(())
}

/// Carves a river whose water follows a descending profile along the complete path. The profile
/// is built from the original ground heights, smoothed over roughly six channel widths, and
/// grade-limited; build this mode over the whole grid so every part of the river shares that
/// profile. `meander_flatten` is ignored in this mode.
///
/// # Errors
/// `Cancelled`, `BadShape`, `BadPath` (fewer than 2 distinct points, or none inside the grid), `BadParameter`.
#[expect(
    clippy::too_many_lines,
    reason = "the path profile and cell carving share this mode's validated inputs"
)]
pub fn carve_river_descending(
    ground: &mut Ground<'_>,
    path: &Path,
    profile: &RiverProfile,
    cancel: &dyn Fn() -> bool,
) -> Result<RiverOut, RiverError> {
    ground.check()?;
    validate_profile(profile)?;
    if cancel() {
        return Err(RiverError::Cancelled);
    }
    let samples = path_samples(path, ground.cell * 0.5)?;
    if !overlaps_grid(&samples, ground) {
        return Err(RiverError::BadPath {
            why: "the path does not cross the grid".into(),
        });
    }
    let original = ground.heights.to_vec();
    let reference: Vec<f32> = samples
        .iter()
        .map(|sample| sample_height(&original, ground, sample.point))
        .collect();
    let mut levels: Vec<f32> = reference
        .iter()
        .map(|height| height - profile.water_level_offset - 0.01)
        .collect();
    let mut running = levels[0];
    for level in &mut levels {
        running = running.min(*level);
        *level = running;
    }
    smooth_levels(&samples, &mut levels, profile.width * 6.0);
    running = levels[0];
    for level in &mut levels {
        running = running.min(*level);
        *level = running;
    }
    // A downstream drop that exceeds the grade is paid for by lowering its upstream approach.
    for index in (0..levels.len().saturating_sub(1)).rev() {
        let run = samples[index + 1].distance - samples[index].distance;
        levels[index] = levels[index].min(levels[index + 1] + 0.05 * run);
    }
    running = levels[0];
    for level in &mut levels {
        running = running.min(*level);
        *level = running;
    }
    for (level, height) in levels.iter_mut().zip(reference) {
        *level = level.min(height - profile.water_level_offset - 0.01);
    }
    running = levels[0];
    for level in &mut levels {
        running = running.min(*level);
        *level = running;
    }
    for index in (0..levels.len().saturating_sub(1)).rev() {
        let run = samples[index + 1].distance - samples[index].distance;
        levels[index] = levels[index].min(levels[index + 1] + 0.05 * run);
    }
    let count = original.len();
    let mut output = RiverOut {
        water_height: original.clone(),
        flow_dir: vec![[0.0, 0.0]; count],
        flow_speed: vec![0.0; count],
        bank_mask: vec![0.0; count],
        channel_mask: vec![0.0; count],
    };
    let half_width = profile.width * 0.5;
    let outer = half_width + profile.bank_width;
    let bank_tangent = tangent(profile.bank_slope_deg);
    let water_depth = (profile.depth - profile.water_level_offset).max(0.1);
    for z in 0..ground.depth {
        if cancel() {
            return Err(RiverError::Cancelled);
        }
        for x in 0..ground.width {
            let index = z * ground.width + x;
            let world = [
                ground.origin[0] + crate::num::cell_f32(x) * ground.cell,
                ground.origin[1] + crate::num::cell_f32(z) * ground.cell,
            ];
            let (distance, direction, along) = nearest(&samples, world);
            if distance > outer {
                continue;
            }
            let level = interpolate_level(&samples, &levels, along);
            let bed = level - water_depth;
            let channel = 1.0 - smoothstep(half_width, half_width + ground.cell, distance);
            let bank = 1.0 - smoothstep(half_width, outer.max(half_width + ground.cell), distance);
            let target = if distance <= half_width {
                bed
            } else {
                (original[index]).min(bed + (distance - half_width) * bank_tangent)
            };
            let blend = if distance <= half_width { 1.0 } else { bank };
            ground.heights[index] = original[index] + (target - original[index]) * blend;
            output.channel_mask[index] = channel;
            output.bank_mask[index] = (bank - channel).max(0.0);
            if distance <= half_width + ground.cell * 1.0e-4 {
                output.water_height[index] = level;
                output.flow_dir[index] = direction;
                output.flow_speed[index] =
                    (0.3 + 12.0 * local_grade(&samples, &levels, along) + 0.05 * water_depth)
                        .min(8.0);
            }
        }
    }
    Ok(output)
}

fn smooth_levels(samples: &[super::geometry::Sample], levels: &mut [f32], window: f32) {
    let source = levels.to_vec();
    let radius = window * 0.5;
    let (mut start, mut end, mut sum) = (0, 0, 0.0);
    for (index, level) in levels.iter_mut().enumerate() {
        let left = samples[index].distance - radius;
        let right = samples[index].distance + radius;
        while start < samples.len() && samples[start].distance < left {
            sum -= source[start];
            start += 1;
        }
        while end < samples.len() && samples[end].distance <= right {
            sum += source[end];
            end += 1;
        }
        *level = sum / crate::num::count_f32(end - start);
    }
}

fn interpolate_level(samples: &[super::geometry::Sample], levels: &[f32], along: f32) -> f32 {
    let index = samples
        .partition_point(|sample| sample.distance < along)
        .min(samples.len() - 1);
    if index == 0 {
        return levels[0];
    }
    let before = index - 1;
    let span = samples[index].distance - samples[before].distance;
    let t = if span > f32::EPSILON {
        (along - samples[before].distance) / span
    } else {
        0.0
    };
    levels[before] * (1.0 - t) + levels[index] * t
}

fn local_grade(samples: &[super::geometry::Sample], levels: &[f32], along: f32) -> f32 {
    let index = samples
        .partition_point(|sample| sample.distance < along)
        .min(samples.len() - 1);
    let pair = if index == 0 {
        (0, 1)
    } else if index >= samples.len() - 1 {
        (index - 1, index)
    } else {
        (index - 1, index + 1)
    };
    let run = samples[pair.1].distance - samples[pair.0].distance;
    if run > f32::EPSILON {
        ((levels[pair.0] - levels[pair.1]) / run).max(0.0)
    } else {
        0.0
    }
}

fn sample_height(heights: &[f32], ground: &Ground<'_>, point: [f32; 2]) -> f32 {
    let x = ((point[0] - ground.origin[0]) / ground.cell)
        .clamp(0.0, crate::num::cell_f32(ground.width - 1));
    let z = ((point[1] - ground.origin[1]) / ground.cell)
        .clamp(0.0, crate::num::cell_f32(ground.depth - 1));
    let x0 = crate::num::floor_index(x);
    let z0 = crate::num::floor_index(z);
    let x1 = (x0 + 1).min(ground.width - 1);
    let z1 = (z0 + 1).min(ground.depth - 1);
    let tx = x - crate::num::cell_f32(x0);
    let tz = z - crate::num::cell_f32(z0);
    let a = heights[z0 * ground.width + x0] * (1.0 - tx) + heights[z0 * ground.width + x1] * tx;
    let b = heights[z1 * ground.width + x0] * (1.0 - tx) + heights[z1 * ground.width + x1] * tx;
    a * (1.0 - tz) + b * tz
}

fn smoothstep(start: f32, end: f32, value: f32) -> f32 {
    let t = ((value - start) / (end - start).max(f32::EPSILON)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn tangent(degrees: f32) -> f32 {
    let radians = degrees * 0.017_453_292;
    let sine =
        radians - radians.powi(3) / 6.0 + radians.powi(5) / 120.0 - radians.powi(7) / 5_040.0;
    let cosine = 1.0 - radians.powi(2) / 2.0 + radians.powi(4) / 24.0 - radians.powi(6) / 720.0;
    sine / cosine
}

fn slope_speed(samples: &[super::geometry::Sample], along: f32) -> f32 {
    let mut before = None;
    let mut after = None;
    for sample in samples {
        if sample.distance <= along {
            before = Some(sample.point);
        } else if after.is_none() {
            after = Some(sample.point);
            break;
        }
    }
    match (before, after) {
        (Some(a), Some(b)) => (a[1] - b[1]).abs() * 0.01,
        _ => 0.0,
    }
}
