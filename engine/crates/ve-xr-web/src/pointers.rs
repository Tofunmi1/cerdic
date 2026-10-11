//! Turns tracked controllers into the pointers the interaction layer understands.

use bevy_math::{Mat4, Vec3};
use ve_interact::{PointerFrame, PointerId, Pose, Ray};

use crate::frame::{Handedness, XrController};

/// A button counts as pressed above this analogue value.
const PRESS_THRESHOLD: f32 = 0.5;

/// Metres per second a held object is pushed along the ray at full thumbstick deflection.
const REACH_SPEED: f32 = 2.0;

/// The thumbstick's y axis in the first four axes of a controller (Touch-style layout).
const THUMBSTICK_Y: usize = 3;

/// The pointer id of a hand: left 1, right 2, anything else 3 (the mouse is 0).
#[must_use]
pub fn pointer_id(hand: Handedness) -> PointerId {
    match hand {
        Handedness::Left => PointerId(1),
        Handedness::Right => PointerId(2),
        Handedness::None => PointerId(3),
    }
}

/// The pointer frame of a controller: it points along its aim ray, is held by its grip, the trigger
/// is the primary button, the squeeze grabs, and pushing the thumbstick forward pushes a held
/// object away. `rig` places tracking space in the world; `dt` is the frame time in seconds.
#[must_use]
pub fn controller_pointer(controller: &XrController, rig: Mat4, dt: f32) -> PointerFrame {
    let ray = controller.aim.and_then(|aim| {
        let world = rig * aim;
        Ray::new(
            world.transform_point3(Vec3::ZERO),
            world.transform_vector3(Vec3::NEG_Z),
        )
    });
    let grip = controller
        .grip
        .and_then(|grip| Pose::from_matrix(rig * grip));
    let stick = controller.axes[THUMBSTICK_Y];
    PointerFrame {
        id: pointer_id(controller.handedness),
        ray,
        grip,
        select: controller.buttons[0] > PRESS_THRESHOLD,
        squeeze: controller.buttons[1] > PRESS_THRESHOLD,
        // Pushing the stick forward reads as negative y.
        reach: -stick * REACH_SPEED * dt.max(0.0),
        select_grabs: false,
    }
}

#[cfg(test)]
mod tests {
    use bevy_math::Quat;

    use super::*;

    fn controller(hand: Handedness) -> XrController {
        XrController {
            handedness: hand,
            grip: Some(Mat4::from_translation(Vec3::new(0.0, 1.0, -0.5))),
            aim: Some(Mat4::from_translation(Vec3::new(0.0, 1.0, -0.5))),
            buttons: [0.0; 8],
            axes: [0.0; 4],
        }
    }

    #[test]
    fn hands_get_distinct_ids_that_are_not_the_mouse() {
        let ids = [Handedness::Left, Handedness::Right, Handedness::None].map(pointer_id);
        assert_eq!(ids, [PointerId(1), PointerId(2), PointerId(3)]);
    }

    #[test]
    fn the_aim_pose_gives_the_ray_in_the_world_through_the_rig() {
        let rig = Mat4::from_translation(Vec3::new(0.0, 0.0, 3.0));
        let frame = controller_pointer(&controller(Handedness::Right), rig, 0.016);
        let ray = frame.ray.unwrap();
        assert!(ray.origin.abs_diff_eq(Vec3::new(0.0, 1.0, 2.5), 1e-6));
        assert!(ray.direction.abs_diff_eq(Vec3::NEG_Z, 1e-6));
        assert!(
            frame
                .grip
                .unwrap()
                .position
                .abs_diff_eq(Vec3::new(0.0, 1.0, 2.5), 1e-6)
        );
        assert_eq!(frame.id, PointerId(2));
        assert!(
            !frame.select_grabs,
            "controllers grab with the squeeze, not the trigger"
        );
    }

    #[test]
    fn a_turned_rig_turns_the_ray() {
        let rig = Mat4::from_quat(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2));
        let mut c = controller(Handedness::Left);
        c.aim = Some(Mat4::IDENTITY);
        let ray = controller_pointer(&c, rig, 0.016).ray.unwrap();
        // A quarter turn about +y takes -z to -x.
        assert!(
            ray.direction.abs_diff_eq(Vec3::NEG_X, 1e-5),
            "{}",
            ray.direction
        );
    }

    #[test]
    fn buttons_press_above_half() {
        let mut c = controller(Handedness::Left);
        c.buttons[0] = 0.4;
        c.buttons[1] = 0.6;
        let frame = controller_pointer(&c, Mat4::IDENTITY, 0.016);
        assert!(!frame.select && frame.squeeze);
        c.buttons[0] = 0.9;
        assert!(controller_pointer(&c, Mat4::IDENTITY, 0.016).select);
    }

    #[test]
    fn pushing_the_stick_forward_reaches_out_in_proportion_to_time() {
        let mut c = controller(Handedness::Right);
        c.axes[3] = -1.0;
        let frame = controller_pointer(&c, Mat4::IDENTITY, 0.5);
        assert!((frame.reach - 1.0).abs() < 1e-6, "{}", frame.reach);
        c.axes[3] = 0.5;
        assert!(
            controller_pointer(&c, Mat4::IDENTITY, 0.5).reach < 0.0,
            "pulling back reaches in"
        );
        assert!(
            controller_pointer(&c, Mat4::IDENTITY, -1.0).reach.abs() < 1e-6,
            "negative dt is ignored"
        );
    }

    #[test]
    fn an_untracked_or_broken_pose_gives_no_ray() {
        let mut c = controller(Handedness::Left);
        c.aim = None;
        c.grip = None;
        let frame = controller_pointer(&c, Mat4::IDENTITY, 0.016);
        assert!(frame.ray.is_none() && frame.grip.is_none());
        c.aim = Some(Mat4::from_cols_array(&[f32::NAN; 16]));
        assert!(controller_pointer(&c, Mat4::IDENTITY, 0.016).ray.is_none());
    }
}
