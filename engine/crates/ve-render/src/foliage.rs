//! Foliage material settings, including the opt-in grass look.
use bevy_reflect::{ReflectDeserialize, ReflectSerialize, std_traits::ReflectDefault};
/// Leaf-like behaviour of a surface: cut away where its albedo map is transparent, and moving in the wind. All zero is an
/// ordinary surface.
#[derive(
    Clone, Copy, Debug, PartialEq, bevy_reflect::Reflect, serde::Serialize, serde::Deserialize,
)]
#[reflect(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FoliageStyle {
    /// Pixels whose albedo-map alpha is below this are discarded (0 keeps everything).
    pub alpha_cutoff: f32,
    /// How far the whole shape bends with the wind, 0 to 1 (more the higher a point is above the shape's origin).
    pub sway: f32,
    /// How much the surface trembles at a small scale, 0 to 1.
    pub flutter: f32,
    /// Enable root shading, upward normals and distance shrink for grass blades.
    pub grass: bool,
    /// Soil colour at the blade roots (sRGB).
    pub ground_color: [f32; 3],
    /// Strength of the soil colour blend, 0 to 1.
    pub ground_blend: f32,
    /// Distance in metres through which blades keep their full size.
    pub fade_near: f32,
    /// Distance in metres where blades shrink to zero; at or below near disables fading.
    pub fade_far: f32,
    /// Light passing through blades facing away from the sun, 0 to 1.
    pub translucency: f32,
    /// Multiplier of the blade's base colour, floored at 0.1.
    pub exposure_boost: f32,
}

impl Default for FoliageStyle {
    fn default() -> Self {
        Self::plain()
    }
}
impl FoliageStyle {
    /// An ordinary surface, with all foliage effects disabled.
    pub const fn plain() -> Self {
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
