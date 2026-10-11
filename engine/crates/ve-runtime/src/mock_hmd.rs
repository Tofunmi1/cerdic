//! A mock headset for desktop: turns a head pose into two eye views each frame.

use bevy_camera::Camera3d;
use bevy_ecs::component::Component;
use bevy_ecs::query::With;
use bevy_ecs::system::{Commands, Query};
use bevy_math::Vec3;
use bevy_transform::components::Transform;
use bevy_window::{PrimaryWindow, Window};
use ve_render::{StereoEye, StereoViews};
use ve_xr_math::{FovTangents, MockHmd, Pose};

/// Distance to the near plane for the mock headset's eyes, in metres.
const NEAR: f32 = 0.05;

/// The head pose and headset description that drive a stereo camera on desktop.
#[derive(Component, Clone, Copy, Debug)]
pub(crate) struct MockHead {
    pub(crate) pose: Pose,
    pub(crate) hmd: MockHmd,
}

/// Builds the mock head used by the stereo scene: standing a little back from the origin, looking
/// at it, with a roughly Quest-like 0.064 m IPD and a wide field of view.
pub(crate) fn default_head(eye: Vec3, target: Vec3) -> MockHead {
    let facing = Transform::from_translation(eye).looking_at(target, Vec3::Y);
    let fov = FovTangents::symmetric(1.6, 1.8).unwrap_or(FovTangents {
        left: -1.0,
        right: 1.0,
        down: -1.0,
        up: 1.0,
    });
    MockHead {
        pose: Pose {
            position: facing.translation,
            orientation: facing.rotation,
        },
        hmd: MockHmd::new(0.064, fov),
    }
}

/// Converts the mock headset's eyes for a target of `size` pixels into the render component.
pub(crate) fn stereo_views(head: &MockHead, size: (u32, u32)) -> StereoViews {
    let [left, right] = head.hmd.eyes(head.pose, size, NEAR);
    let eye = |view: ve_xr_math::EyeView| StereoEye {
        world_from_view: view.world_from_view,
        clip_from_view: view.clip_from_view,
        viewport: [
            view.viewport.x,
            view.viewport.y,
            view.viewport.width,
            view.viewport.height,
        ],
    };
    StereoViews {
        eyes: [eye(left), eye(right)],
    }
}

/// Keeps every mock-headset camera's eyes in step with the window size.
pub(crate) fn update_mock_hmd(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut rigs: Query<(&MockHead, &mut StereoViews)>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let size = (
        window.resolution.physical_width(),
        window.resolution.physical_height(),
    );
    for (head, mut views) in &mut rigs {
        *views = stereo_views(head, size);
    }
}

/// Spawns the stereo camera: a normal `Camera3d` plus the mock head and a placeholder pair of eyes
/// that `update_mock_hmd` replaces on the first frame.
pub(crate) fn spawn_stereo_camera(commands: &mut Commands, head: MockHead) {
    commands.spawn((Camera3d::default(), head, stereo_views(&head, (2, 1))));
}
