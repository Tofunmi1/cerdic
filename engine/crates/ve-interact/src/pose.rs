//! Poses used by interaction: where something is, how it is turned and (for held objects) how big.

use glam::{Mat4, Quat, Vec3};

/// Position and orientation, for pointers and hands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    /// Position in world space.
    pub position: Vec3,
    /// Orientation in world space.
    pub rotation: Quat,
}

impl Pose {
    /// The pose of a transform matrix (any scale is ignored), or `None` if it is not finite.
    #[must_use]
    pub fn from_matrix(matrix: Mat4) -> Option<Self> {
        if !matrix.is_finite() {
            return None;
        }
        let (_, rotation, position) = matrix.to_scale_rotation_translation();
        Some(Self {
            position,
            rotation: rotation.normalize(),
        })
    }
}

/// The placement of an interactive object: translation, rotation and one uniform scale factor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement3 {
    /// Position in world space.
    pub translation: Vec3,
    /// Orientation in world space.
    pub rotation: Quat,
    /// Uniform scale multiplier (1 is the object's authored size).
    pub scale: f32,
}

impl Placement3 {
    /// The identity placement.
    pub const IDENTITY: Self = Self {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: 1.0,
    };

    /// The placement as a matrix.
    #[must_use]
    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            Vec3::splat(self.scale),
            self.rotation,
            self.translation,
        )
    }

    /// Whether every component is a finite number.
    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.translation.is_finite() && self.rotation.is_finite() && self.scale.is_finite()
    }

    /// Whether two placements are equal to within `epsilon` in position, orientation and scale.
    #[must_use]
    pub fn approx_eq(&self, other: &Self, epsilon: f32) -> bool {
        self.translation.abs_diff_eq(other.translation, epsilon)
            && self.rotation.abs_diff_eq(other.rotation, epsilon)
            && (self.scale - other.scale).abs() <= epsilon
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pose_ignores_scale() {
        let matrix = Mat4::from_scale_rotation_translation(
            Vec3::splat(3.0),
            Quat::from_rotation_y(0.5),
            Vec3::new(1.0, 2.0, 3.0),
        );
        let pose = Pose::from_matrix(matrix).unwrap();
        assert!(pose.position.abs_diff_eq(Vec3::new(1.0, 2.0, 3.0), 1e-6));
        assert!(pose.rotation.abs_diff_eq(Quat::from_rotation_y(0.5), 1e-5));
        assert!(Pose::from_matrix(Mat4::from_cols_array(&[f32::NAN; 16])).is_none());
    }

    #[test]
    fn placement_matrix_round_trips() {
        let placement = Placement3 {
            translation: Vec3::new(1.0, -2.0, 0.5),
            rotation: Quat::from_rotation_x(1.0),
            scale: 2.0,
        };
        let (scale, rotation, translation) = placement.matrix().to_scale_rotation_translation();
        assert!((scale.x - 2.0).abs() < 1e-5 && (scale.y - 2.0).abs() < 1e-5);
        assert!(rotation.abs_diff_eq(placement.rotation, 1e-5));
        assert!(translation.abs_diff_eq(placement.translation, 1e-6));
        assert!(placement.is_finite());
        assert!(
            !Placement3 {
                scale: f32::NAN,
                ..placement
            }
            .is_finite()
        );
    }
}
