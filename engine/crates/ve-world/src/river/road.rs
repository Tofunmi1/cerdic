//! Carving a grade-limited road corridor.
use super::geometry::{nearest, overlaps_grid, path_samples};
use super::{Ground, Path, RiverError};

/// The cross-section of a road.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoadProfile {
    /// Width of the road surface in metres.
    pub width: f32,
    /// Width of the shoulder on each side in metres.
    pub shoulder: f32,
    /// Steepest allowed grade along the road, in percent (rise over run times 100), at most 40.
    pub max_grade_percent: f32,
    /// Cross-slope for drainage: the centre is this many metres higher than the edges.
    pub crown: f32,
    /// Angle of the cut and fill embankments from flat, in degrees, 5..=85.
    pub embank_slope_deg: f32,
}

/// What a road carve produces, one value per cell of the ground.
#[derive(Debug, Clone, PartialEq)]
pub struct RoadOut {
    /// 0..=1, the road surface.
    pub road_mask: Vec<f32>,
    /// 0..=1, the shoulders and embankments.
    pub shoulder_mask: Vec<f32>,
    /// Total volume of earth moved, in cubic metres.
    pub cut_fill: f32,
}

/// Flattens a corridor along `path`: the road height follows the terrain along the whole path but its grade never exceeds
/// `max_grade_percent` (a smoothed profile), and cut and fill blend into the ground with the embankment angle. The profile runs along
/// the whole path, so build a road on the whole grid (or hand every tile the same full-path profile).
///
/// # Errors
/// `Cancelled`, `BadShape`, `BadPath`, `BadParameter` (grade over 40 percent, angles outside 5..=85, non-positive width).
pub fn carve_road(
    ground: &mut Ground<'_>,
    path: &Path,
    profile: &RoadProfile,
    cancel: &dyn Fn() -> bool,
) -> Result<RoadOut, RiverError> {
    ground.check()?;
    validate_profile(profile)?;
    if cancel() {
        return Err(RiverError::Cancelled);
    }
    let mut samples = path_samples(path, 1.0)?;
    if !overlaps_grid(&samples, ground) {
        return Err(RiverError::BadPath {
            why: "the path does not cross the grid".into(),
        });
    }
    let original = ground.heights.to_vec();
    let mut profile_heights: Vec<f32> = samples
        .iter()
        .map(|sample| sample_height(&original, ground, sample.point))
        .collect();
    smooth_profile(&mut profile_heights);
    limit_grade(
        &mut samples,
        &mut profile_heights,
        profile.max_grade_percent / 100.0,
    );
    let mut road_mask = vec![0.0; original.len()];
    let mut shoulder_mask = vec![0.0; original.len()];
    let mut cut_fill = 0.0;
    for z in 0..ground.depth {
        if cancel() {
            return Err(RiverError::Cancelled);
        }
        carve_row(
            ground,
            z,
            &original,
            &samples,
            &profile_heights,
            profile,
            &mut road_mask,
            &mut shoulder_mask,
            &mut cut_fill,
        );
    }
    Ok(RoadOut {
        road_mask,
        shoulder_mask,
        cut_fill,
    })
}

fn carve_row(
    ground: &mut Ground<'_>,
    z: usize,
    original: &[f32],
    samples: &[super::geometry::Sample],
    profile_heights: &[f32],
    profile: &RoadProfile,
    road_mask: &mut [f32],
    shoulder_mask: &mut [f32],
    cut_fill: &mut f32,
) {
    let half_width = profile.width * 0.5;
    let shoulder_edge = half_width + profile.shoulder;
    let embank = tangent(profile.embank_slope_deg);
    for x in 0..ground.width {
        let index = z * ground.width + x;
        let world = [
            ground.origin[0] + crate::num::cell_f32(x) * ground.cell,
            ground.origin[1] + crate::num::cell_f32(z) * ground.cell,
        ];
        let (distance, _direction, along) = nearest(samples, world);
        let center = interpolate_profile(samples, profile_heights, along);
        let surface = center + profile.crown * (1.0 - (distance / half_width).clamp(0.0, 1.0));
        let falloff = (surface - original[index]).abs() / embank;
        let outside = shoulder_edge + falloff;
        if distance > outside || distance > shoulder_edge && falloff <= f32::EPSILON {
            continue;
        }
        let target = if distance <= shoulder_edge {
            surface
        } else {
            let ratio =
                1.0 - (distance - shoulder_edge) / (outside - shoulder_edge).max(f32::EPSILON);
            original[index] + (surface - original[index]) * ratio.clamp(0.0, 1.0)
        };
        let blend = if distance <= half_width {
            1.0
        } else {
            1.0 - smoothstep(
                shoulder_edge,
                outside.max(shoulder_edge + ground.cell),
                distance,
            )
        };
        let changed = original[index] + (target - original[index]) * blend;
        ground.heights[index] = changed;
        *cut_fill += (changed - original[index]).abs() * ground.cell * ground.cell;
        road_mask[index] = 1.0
            - smoothstep(
                half_width - ground.cell * 0.5,
                half_width + ground.cell * 0.5,
                distance,
            );
        shoulder_mask[index] = (1.0
            - smoothstep(
                half_width,
                shoulder_edge.max(half_width + ground.cell),
                distance,
            ))
        .max((1.0 - road_mask[index]) * blend);
    }
}

fn validate_profile(profile: &RoadProfile) -> Result<(), RiverError> {
    for (name, value, positive) in [
        ("width", profile.width, true),
        ("shoulder", profile.shoulder, false),
        ("crown", profile.crown, false),
        ("max_grade_percent", profile.max_grade_percent, false),
    ] {
        if !value.is_finite() || value < 0.0 || (positive && value <= 0.0) {
            return Err(RiverError::BadParameter {
                name,
                why: "the value must be finite and non-negative (width must be positive)".into(),
            });
        }
    }
    if profile.max_grade_percent > 40.0 {
        return Err(RiverError::BadParameter {
            name: "max_grade_percent",
            why: "the maximum grade is 40 percent".into(),
        });
    }
    if !(5.0..=85.0).contains(&profile.embank_slope_deg) {
        return Err(RiverError::BadParameter {
            name: "embank_slope_deg",
            why: "the angle must be in 5..=85 degrees".into(),
        });
    }
    Ok(())
}

fn smooth_profile(values: &mut [f32]) {
    let original = values.to_vec();
    for (index, value) in values.iter_mut().enumerate() {
        let start = index.saturating_sub(2);
        let end = (index + 3).min(original.len());
        let sum: f32 = original[start..end].iter().sum();
        *value = sum / crate::num::count_f32(end - start);
    }
}

fn limit_grade(samples: &mut [super::geometry::Sample], values: &mut [f32], grade: f32) {
    for index in 1..values.len() {
        let run = samples[index].distance - samples[index - 1].distance;
        let limit = grade * run;
        values[index] = values[index].clamp(values[index - 1] - limit, values[index - 1] + limit);
    }
    for index in (0..values.len().saturating_sub(1)).rev() {
        let run = samples[index + 1].distance - samples[index].distance;
        let limit = grade * run;
        values[index] = values[index].clamp(values[index + 1] - limit, values[index + 1] + limit);
    }
}

fn interpolate_profile(samples: &[super::geometry::Sample], values: &[f32], along: f32) -> f32 {
    let index = samples
        .partition_point(|sample| sample.distance < along)
        .min(samples.len() - 1);
    if index == 0 {
        return values[0];
    }
    let before = index - 1;
    let span = samples[index].distance - samples[before].distance;
    let t = if span > f32::EPSILON {
        (along - samples[before].distance) / span
    } else {
        0.0
    };
    values[before] * (1.0 - t) + values[index] * t
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
