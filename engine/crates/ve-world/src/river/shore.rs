//! Shore masks around a water level.
use super::{GroundRef, RiverError};

/// Masks around a water level, one value per cell.
#[derive(Debug, Clone, PartialEq)]
pub struct ShoreOut {
    /// 1 where the ground is below the water level, else 0.
    pub underwater: Vec<f32>,
    /// 1 at the water line falling to 0 at `beach_width` metres of height above it (sand, wet earth).
    pub beach: Vec<f32>,
    /// The steepness (0..=1 as in the slope mask) of ground within 1.5 m of height above the water line; 0 elsewhere.
    pub wet_rock: Vec<f32>,
    /// How far the ground is below the level, in metres; 0 above it.
    pub depth_below: Vec<f32>,
}

/// Masks for a water surface at height `level`.
///
/// # Errors
/// `Cancelled`, `BadShape`, `BadParameter` (negative `beach_width`).
pub fn shore_masks(
    ground: &GroundRef<'_>,
    level: f32,
    beach_width: f32,
    cancel: &dyn Fn() -> bool,
) -> Result<ShoreOut, RiverError> {
    ground.check()?;
    if !beach_width.is_finite() || beach_width < 0.0 {
        return Err(RiverError::BadParameter {
            name: "beach_width",
            why: "the width must be finite and non-negative".into(),
        });
    }
    if !level.is_finite() {
        return Err(RiverError::BadParameter {
            name: "level",
            why: "the level must be finite".into(),
        });
    }
    let count = ground.heights.len();
    let mut output = ShoreOut {
        underwater: vec![0.0; count],
        beach: vec![0.0; count],
        wet_rock: vec![0.0; count],
        depth_below: vec![0.0; count],
    };
    for z in 0..ground.depth {
        if cancel() {
            return Err(RiverError::Cancelled);
        }
        for x in 0..ground.width {
            let index = z * ground.width + x;
            let height = ground.heights[index];
            let below = (level - height).max(0.0);
            output.underwater[index] = f32::from(height < level);
            output.depth_below[index] = below;
            if height >= level && beach_width > 0.0 {
                output.beach[index] = 1.0 - smoothstep(0.0, beach_width, height - level);
            }
            let left = ground.heights[z * ground.width + x.saturating_sub(1)];
            let right = ground.heights[z * ground.width + (x + 1).min(ground.width - 1)];
            let up = ground.heights[z.saturating_sub(1) * ground.width + x];
            let down = ground.heights[(z + 1).min(ground.depth - 1) * ground.width + x];
            let slope = (((right - left) / (2.0 * ground.cell)).powi(2)
                + ((down - up) / (2.0 * ground.cell)).powi(2))
            .sqrt();
            let near_line = height >= level && height <= level + 1.5;
            let cliff_foot = [left, right, up, down]
                .iter()
                .any(|neighbor| *neighbor <= level + 1.5)
                && height >= level;
            if near_line || cliff_foot {
                output.wet_rock[index] = smoothstep(1.0, 2.0, slope);
            }
        }
    }
    Ok(output)
}

fn smoothstep(start: f32, end: f32, value: f32) -> f32 {
    let t = ((value - start) / (end - start).max(f32::EPSILON)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
