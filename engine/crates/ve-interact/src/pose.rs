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

/// Smallest axis scale a transform may have and still count as having a surface to point at.
const MIN_AXIS_SCALE: f32 = 1.0e-6;

/// Largest sine of the angle between two axes still treated as perpendicular. Beyond it the
/// transform shears, and a box cannot represent it exactly.
const MAX_SHEAR: f32 = 1.0e-3;

/// A transform split into scale, rotation and translation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Decomposed {
    pub(crate) scale: Vec3,
    pub(crate) rotation: Quat,
    pub(crate) translation: Vec3,
}

/// Splits a matrix into scale, rotation and translation, or `None` when that cannot be done
/// faithfully: non-finite entries, an axis collapsed to (almost) nothing, a mirroring (negative
/// determinant, which a rotation cannot express) or shear (axes that are not perpendicular, as when
/// a rotated child sits under a non-uniformly scaled parent).
pub(crate) fn decompose(matrix: Mat4) -> Option<Decomposed> {
    if !matrix.is_finite() {
        return None;
    }
    let axes = [
        matrix.x_axis.truncate(),
        matrix.y_axis.truncate(),
        matrix.z_axis.truncate(),
    ];
    let lengths = axes.map(Vec3::length);
    if lengths
        .iter()
        .any(|l| !l.is_finite() || *l < MIN_AXIS_SCALE)
    {
        return None;
    }
    for (a, b) in [(0, 1), (0, 2), (1, 2)] {
        let cosine = axes[a].dot(axes[b]) / (lengths[a] * lengths[b]);
        if cosine.abs() > MAX_SHEAR {
            return None;
        }
    }
    if axes[0].cross(axes[1]).dot(axes[2]) <= 0.0 {
        return None;
    }
    let (scale, rotation, translation) = matrix.to_scale_rotation_translation();
    let rotation = rotation.normalize();
    (rotation.is_finite() && scale.is_finite() && translation.is_finite()).then_some(Decomposed {
        scale,
        rotation,
        translation,
    })
}

/// Whether two quaternions turn things the same way. `q` and `-q` are the same rotation, so plain
/// equality is the wrong test.
#[must_use]
pub fn rotations_equivalent(a: Quat, b: Quat) -> bool {
    a.is_finite() && b.is_finite() && a.normalize().dot(b.normalize()).abs() > 1.0 - 1.0e-6
}

impl Pose {
    /// The pose of a transform matrix (any scale is ignored), or `None` if the matrix is not finite
    /// or cannot be split into a rotation and a position (collapsed, mirrored or sheared).
    #[must_use]
    pub fn from_matrix(matrix: Mat4) -> Option<Self> {
        let parts = decompose(matrix)?;
        Some(Self {
            position: parts.translation,
            rotation: parts.rotation,
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
        // `q` and `-q` are the same orientation, so compare through the dot product.
        self.translation.abs_diff_eq(other.translation, epsilon)
            && 1.0
                - self
                    .rotation
                    .normalize()
                    .dot(other.rotation.normalize())
                    .abs()
                <= epsilon
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
    fn degenerate_mirrored_and_sheared_matrices_have_no_pose() {
        let scaled = |s: Vec3| Mat4::from_scale(s);
        assert!(decompose(scaled(Vec3::ONE)).is_some());
        assert!(decompose(scaled(Vec3::new(2.0, 1.0, 0.5))).is_some());
        assert!(
            Pose::from_matrix(scaled(Vec3::ZERO)).is_none(),
            "zero scale"
        );
        assert!(Pose::from_matrix(scaled(Vec3::new(1.0, 0.0, 1.0))).is_none());
        assert!(
            Pose::from_matrix(scaled(Vec3::new(-1.0, 1.0, 1.0))).is_none(),
            "mirror"
        );
        assert!(Pose::from_matrix(scaled(Vec3::new(1.0e-9, 1.0, 1.0))).is_none());
        // A rotated child under a non-uniformly scaled parent shears: its axes are not perpendicular.
        let sheared = scaled(Vec3::new(3.0, 1.0, 1.0)) * Mat4::from_rotation_z(0.6);
        assert!(decompose(sheared).is_none(), "shear");
        assert!(Pose::from_matrix(sheared).is_none());
    }

    #[test]
    fn the_decomposition_always_gives_a_unit_rotation() {
        let matrix = Mat4::from_scale_rotation_translation(
            Vec3::new(2.0, 3.0, 0.5),
            Quat::from_euler(glam::EulerRot::XYZ, 0.4, -1.1, 2.0),
            Vec3::new(5.0, 6.0, 7.0),
        );
        let parts = decompose(matrix).unwrap();
        assert!((parts.rotation.length() - 1.0).abs() < 1e-5);
        assert!(parts.scale.abs_diff_eq(Vec3::new(2.0, 3.0, 0.5), 1e-4));
    }

    #[test]
    fn opposite_quaternions_are_the_same_rotation() {
        let q = Quat::from_rotation_y(0.8);
        assert!(rotations_equivalent(q, -q));
        assert!(rotations_equivalent(q, q));
        assert!(!rotations_equivalent(q, Quat::from_rotation_y(0.9)));
        assert!(!rotations_equivalent(
            q,
            Quat::from_xyzw(f32::NAN, 0.0, 0.0, 1.0)
        ));
        let a = Placement3 {
            rotation: q,
            ..Placement3::IDENTITY
        };
        let b = Placement3 {
            rotation: -q,
            ..Placement3::IDENTITY
        };
        assert!(a.approx_eq(&b, 1e-5), "q and -q are one placement");
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
