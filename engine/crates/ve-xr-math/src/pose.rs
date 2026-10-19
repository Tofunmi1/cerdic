//! Head poses, eye placement and the mock headset used on desktop.

use glam::{Mat4, Quat, Vec3};

use crate::{FovTangents, Viewport, side_by_side};

/// A position and orientation in world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    /// Where the pose is, in metres.
    pub position: Vec3,
    /// Which way it faces; the identity looks down -Z.
    pub orientation: Quat,
}

impl Pose {
    /// The pose at the origin looking down -Z.
    pub const IDENTITY: Self = Self {
        position: Vec3::ZERO,
        orientation: Quat::IDENTITY,
    };

    /// This pose as a world-from-local matrix.
    #[must_use]
    pub fn to_matrix(&self) -> Mat4 {
        Mat4::from_rotation_translation(self.orientation.normalize(), self.position)
    }
}

/// The pose of an eye that sits `offset` (in head space) from `head`.
#[must_use]
pub fn eye_pose(head: Pose, offset: Vec3) -> Pose {
    let orientation = head.orientation.normalize();
    Pose {
        position: head.position + orientation * offset,
        orientation,
    }
}

/// Everything the renderer needs to draw one eye.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EyeView {
    /// The eye's pose as a world-from-view matrix.
    pub world_from_view: Mat4,
    /// The reverse-z projection for this eye's field of view.
    pub clip_from_view: Mat4,
    /// The pixels of the shared target this eye draws into.
    pub viewport: Viewport,
}

/// How far each eye's field of view leans outward, as a fraction of the half-width (0.15 gives about 15 percent more
/// view on the outer side than on the nose side, like a Quest).
const NASAL_SHIFT: f32 = 0.15;

/// A stand-in headset for desktop development and tests: two eyes a fixed distance apart, each with the same field of
/// view leaning outward (see [`NASAL_SHIFT`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MockHmd {
    /// Distance between the eyes in metres. Typical adults are 0.058 to 0.072.
    pub ipd: f32,
    /// Field of view of each eye.
    pub fov: FovTangents,
}

impl MockHmd {
    /// A headset with the given interpupillary distance and field of view.
    #[must_use]
    pub fn new(ipd: f32, fov: FovTangents) -> Self {
        Self { ipd, fov }
    }

    /// The two eye views (left, right) for `head`, laid out side by side on a target of the given
    /// size.
    #[must_use]
    pub fn eyes(&self, head: Pose, target: (u32, u32), near: f32) -> [EyeView; 2] {
        let half = self.ipd * 0.5;
        let [left, right] = side_by_side(target.0, target.1);
        // Real headsets show each eye more to the outside than toward the nose, so the two views overlap in the middle
        // rather than being the same picture. The window keeps its width (so the image is not stretched) and slides
        // outward: the left eye's window moves left, the right eye's moves right.
        let outward = |sign: f32| {
            let shift = |edge: f32, grows: f32| edge * (1.0 + grows * NASAL_SHIFT);
            FovTangents {
                left: shift(self.fov.left, sign),
                right: shift(self.fov.right, -sign),
                down: self.fov.down,
                up: self.fov.up,
            }
            .projection_reverse_z(near)
        };
        let eye = |offset_x: f32, sign: f32, viewport: Viewport| EyeView {
            world_from_view: eye_pose(head, Vec3::new(offset_x, 0.0, 0.0)).to_matrix(),
            clip_from_view: outward(sign),
            viewport,
        };
        [eye(-half, 1.0, left), eye(half, -1.0, right)]
    }
}

#[cfg(test)]
mod tests {
    use core::f32::consts::FRAC_PI_2;

    use proptest::prelude::*;

    use super::*;

    fn hmd() -> MockHmd {
        MockHmd::new(0.064, FovTangents::symmetric(1.8, 1.8).unwrap())
    }

    #[test]
    fn eyes_sit_half_an_ipd_either_side_of_the_head() {
        let [left, right] = hmd().eyes(Pose::IDENTITY, (2000, 1000), 0.05);
        let lp = left.world_from_view.w_axis.truncate();
        let rp = right.world_from_view.w_axis.truncate();
        assert!((lp - Vec3::new(-0.032, 0.0, 0.0)).length() < 1e-6);
        assert!((rp - Vec3::new(0.032, 0.0, 0.0)).length() < 1e-6);
        assert!(((rp - lp).length() - 0.064).abs() < 1e-6);
    }

    #[test]
    fn turning_the_head_turns_the_eye_baseline_with_it() {
        // Yaw a quarter turn: the head's right (+X) now points toward -Z.
        let head = Pose {
            position: Vec3::new(1.0, 1.6, 2.0),
            orientation: Quat::from_rotation_y(FRAC_PI_2),
        };
        let [left, right] = hmd().eyes(head, (2000, 1000), 0.05);
        let lp = left.world_from_view.w_axis.truncate();
        let rp = right.world_from_view.w_axis.truncate();
        let mid = (lp + rp) * 0.5;
        assert!(
            (mid - head.position).length() < 1e-6,
            "eyes centre on the head"
        );
        let baseline = (rp - lp).normalize();
        assert!((baseline - Vec3::NEG_Z).length() < 1e-5, "{baseline:?}");
    }

    #[test]
    fn eyes_mirror_each_other_and_tile_the_target() {
        let [left, right] = hmd().eyes(Pose::IDENTITY, (1001, 500), 0.05);
        assert_ne!(
            left.clip_from_view, right.clip_from_view,
            "the eyes do not see the same window"
        );
        // Mirror images: the horizontal terms of one are the negatives of the other's, the vertical ones are equal.
        let (l, r) = (left.clip_from_view, right.clip_from_view);
        assert!((l.x_axis.x - r.x_axis.x).abs() < 1e-6 && (l.y_axis.y - r.y_axis.y).abs() < 1e-6);
        assert!(
            (l.z_axis.x + r.z_axis.x).abs() < 1e-5,
            "{} {}",
            l.z_axis.x,
            r.z_axis.x
        );
        assert_eq!(left.viewport.width + right.viewport.width, 1001);
        assert_eq!(left.viewport.x + left.viewport.width, right.viewport.x);
    }

    #[test]
    fn an_unnormalised_orientation_is_normalised() {
        let head = Pose {
            position: Vec3::ZERO,
            orientation: Quat::from_xyzw(0.0, 0.0, 0.0, 5.0),
        };
        let eye = eye_pose(head, Vec3::X);
        assert!((eye.position - Vec3::X).length() < 1e-6);
    }

    proptest! {
        #[test]
        fn eye_separation_equals_ipd_for_any_head_pose(
            ipd in 0.03_f32..0.1,
            yaw in -3.0_f32..3.0, pitch in -1.4_f32..1.4, roll in -1.0_f32..1.0,
            x in -10.0_f32..10.0, y in -10.0_f32..10.0, z in -10.0_f32..10.0,
        ) {
            let head = Pose {
                position: Vec3::new(x, y, z),
                orientation: Quat::from_euler(glam::EulerRot::YXZ, yaw, pitch, roll),
            };
            let hmd = MockHmd::new(ipd, FovTangents::symmetric(1.7, 1.7).unwrap());
            let [left, right] = hmd.eyes(head, (1600, 900), 0.05);
            let separation = (right.world_from_view.w_axis - left.world_from_view.w_axis)
                .truncate()
                .length();
            prop_assert!((separation - ipd).abs() < 1e-4);
        }
    }
}
