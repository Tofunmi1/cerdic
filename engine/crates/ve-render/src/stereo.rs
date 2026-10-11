//! Stereo rendering: one camera drawing two eyes into viewports of a shared target.

use bevy_ecs::component::Component;
use bevy_math::Mat4;
use bevy_render::RenderApp;
use bevy_render::extract_component::ExtractComponent;

/// One eye of a stereo view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StereoEye {
    /// The eye's pose as a world-from-view matrix.
    pub world_from_view: Mat4,
    /// The eye's projection, reverse-z with an infinite far plane (see `ve-xr-math`).
    pub clip_from_view: Mat4,
    /// The pixels of the camera's target this eye draws into: `[x, y, width, height]`, origin at
    /// the top left.
    pub viewport: [u32; 4],
}

/// Add to a `Camera3d` to render it in stereo.
///
/// The camera's own transform and projection are ignored; the two eyes below replace them. Both
/// eyes are drawn in a single render pass into one multisampled target, so MSAA resolves once and
/// the instance data is shared. The application updates the eyes every frame (from a headset
/// pose, or from a mock headset on desktop).
#[derive(Component, ExtractComponent, Clone, Copy, Debug, PartialEq)]
#[extract_app(RenderApp)]
pub struct StereoViews {
    /// Left and right eye.
    pub eyes: [StereoEye; 2],
}
