//! Render quality knobs. The tier table that sets them from a device budget arrives in M6; for now
//! they are plain settings with safe defaults.

use bevy_ecs::prelude::*;
use bevy_render::Extract;

use crate::light::MAX_POINT_LIGHTS;

/// Settings that trade image quality for speed.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderQuality {
    /// Point lights shaded per frame (the closest to the viewer are kept).
    pub max_point_lights: usize,
}

impl Default for RenderQuality {
    fn default() -> Self {
        Self {
            max_point_lights: MAX_POINT_LIGHTS,
        }
    }
}

/// Copies the main-world [`RenderQuality`] into the render world each frame.
pub(crate) fn extract_quality(mut commands: Commands, quality: Extract<Res<RenderQuality>>) {
    commands.insert_resource(**quality);
}
