//! Stereo rendering: one camera drawing two eyes into viewports of a shared target.

use bevy_ecs::component::Component;
use bevy_math::{Mat4, Vec3};
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

/// Where the viewer is, for choosing lights and fitting shadows: the point between the eyes of a
/// stereo camera (the pose the headset reports, not the transform of the desktop camera that
/// carries it), or the position of a plain camera. Each item is a camera's own position and its
/// stereo eyes if it has any. A stereo camera wins over plain ones, because it is what the user
/// sees; among equals the first wins. With no camera the origin is used.
pub(crate) fn viewer_position<'a>(
    views: impl IntoIterator<Item = (Vec3, Option<&'a StereoViews>)>,
) -> Vec3 {
    let mut plain = None;
    for (position, stereo) in views {
        if let Some(stereo) = stereo {
            let [left, right] = &stereo.eyes;
            return (left.world_from_view.w_axis.truncate()
                + right.world_from_view.w_axis.truncate())
                * 0.5;
        }
        plain.get_or_insert(position);
    }
    plain.unwrap_or(Vec3::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eyes(left_x: f32, right_x: f32, y: f32) -> StereoViews {
        let eye = |x: f32| StereoEye {
            world_from_view: Mat4::from_translation(Vec3::new(x, y, 5.0)),
            clip_from_view: Mat4::IDENTITY,
            viewport: [0, 0, 1, 1],
        };
        StereoViews {
            eyes: [eye(left_x), eye(right_x)],
        }
    }

    #[test]
    fn a_stereo_camera_is_located_between_its_eyes_not_at_its_transform() {
        let stereo = eyes(2.0, 2.06, 1.6);
        let at = viewer_position([(Vec3::new(100.0, 0.0, 0.0), Some(&stereo))]);
        assert!(at.abs_diff_eq(Vec3::new(2.03, 1.6, 5.0), 1e-6), "{at}");
    }

    #[test]
    fn stereo_wins_over_a_plain_camera_in_any_order() {
        let stereo = eyes(0.0, 0.0, 1.0);
        let plain = (Vec3::new(9.0, 9.0, 9.0), None);
        let with_stereo = (Vec3::ZERO, Some(&stereo));
        let a = viewer_position([plain, with_stereo]);
        let b = viewer_position([with_stereo, plain]);
        assert!(a.abs_diff_eq(Vec3::new(0.0, 1.0, 5.0), 1e-6) && a == b);
    }

    #[test]
    fn plain_cameras_and_no_cameras() {
        assert_eq!(
            viewer_position([(Vec3::X, None), (Vec3::Y, None)]),
            Vec3::X,
            "the first plain camera"
        );
        assert_eq!(viewer_position([]), Vec3::ZERO);
    }
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
