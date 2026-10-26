//! Serializable foliage and grass settings.
use serde::{Deserialize, Serialize};
/// How a leaf-like surface is cut out and moves, from a `solid.foliage` node.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Foliage {
    /// The surface is cut away where its albedo map's alpha is below this (0 keeps every pixel, 1 keeps only the opaque ones).
    pub alpha_cutoff: f32,
    /// How far the whole shape bends with the wind, 0 still to 1 swaying strongly; stronger the higher it is above its base.
    pub sway: f32,
    /// How much the surface flutters in the wind at a smaller scale (leaves trembling), 0 to 1.
    pub flutter: f32,
    /// Enable the grass look: shaded roots, upward normals and distance shrink.
    pub grass: bool,
    /// Soil colour at the roots (sRGB).
    pub ground_color: [f32; 3],
    /// Root colour blending strength, 0 to 1.
    pub ground_blend: f32,
    /// Full size through this distance, in metres.
    pub fade_near: f32,
    /// Shrink to zero at this distance, in metres; at or below near disables fading.
    pub fade_far: f32,
    /// Light transmitted through the blade, 0 to 1.
    pub translucency: f32,
    /// Base colour multiplier, floored at 0.1 in the renderer.
    pub exposure_boost: f32,
}

impl Default for Foliage {
    fn default() -> Self {
        Self {
            alpha_cutoff: 0.0,
            sway: 0.0,
            flutter: 0.0,
            grass: false,
            ground_color: [0.25, 0.18, 0.10],
            ground_blend: 0.0,
            fade_near: 25.0,
            fade_far: 70.0,
            translucency: 0.0,
            exposure_boost: 1.0,
        }
    }
}
