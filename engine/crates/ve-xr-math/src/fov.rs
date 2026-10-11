//! Asymmetric fields of view and the reverse-z projections built from them.

use glam::{Mat4, Vec4};

/// Why a field of view was rejected.
#[derive(Clone, Copy, Debug, PartialEq, thiserror::Error)]
pub enum FovError {
    /// A tangent was NaN or infinite.
    #[error("field of view contains a non-finite value")]
    NonFinite,
    /// `left` must be less than `right`, and `down` less than `up`.
    #[error("field of view edges are not ordered (need left < right and down < up)")]
    Unordered,
}

/// The four edges of a view frustum as tangents of the angle from the view axis, measured at unit
/// distance. Left and down are negative for a frustum that straddles the axis, so a symmetric
/// 90 degree field of view is `{ left: -1, right: 1, down: -1, up: 1 }`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FovTangents {
    /// Tangent of the left edge (usually negative).
    pub left: f32,
    /// Tangent of the right edge (usually positive).
    pub right: f32,
    /// Tangent of the bottom edge (usually negative).
    pub down: f32,
    /// Tangent of the top edge (usually positive).
    pub up: f32,
}

impl FovTangents {
    /// Validates and wraps the four tangents.
    ///
    /// # Errors
    /// [`FovError::NonFinite`] for NaN or infinity, [`FovError::Unordered`] if an edge pair is
    /// reversed or empty.
    pub fn new(left: f32, right: f32, down: f32, up: f32) -> Result<Self, FovError> {
        if ![left, right, down, up].iter().all(|v| v.is_finite()) {
            return Err(FovError::NonFinite);
        }
        if left >= right || down >= up {
            return Err(FovError::Unordered);
        }
        Ok(Self {
            left,
            right,
            down,
            up,
        })
    }

    /// A frustum centred on the view axis with the given full horizontal and vertical angles.
    ///
    /// # Errors
    /// [`FovError`] if an angle is not finite or not in (0, pi).
    pub fn symmetric(horizontal_radians: f32, vertical_radians: f32) -> Result<Self, FovError> {
        let in_range = |a: f32| a.is_finite() && a > 0.0 && a < core::f32::consts::PI;
        if !(in_range(horizontal_radians) && in_range(vertical_radians)) {
            return Err(FovError::NonFinite);
        }
        let x = (horizontal_radians * 0.5).tan();
        let y = (vertical_radians * 0.5).tan();
        Self::new(-x, x, -y, y)
    }

    /// Recovers the frustum edges from an OpenGL-style projection matrix (column-major, as
    /// `XRView.projectionMatrix` provides). Returns `None` if the matrix is not a perspective
    /// projection or yields an invalid frustum.
    #[must_use]
    pub fn from_gl_projection(m: &[f32; 16]) -> Option<Self> {
        // Perspective projections have m[11] == -1 and no x/y/z mixing into the wrong rows.
        if (m[11] + 1.0).abs() > 1e-3 || m[0].abs() < f32::EPSILON || m[5].abs() < f32::EPSILON {
            return None;
        }
        let right = (1.0 + m[8]) / m[0];
        let left = (m[8] - 1.0) / m[0];
        let up = (1.0 + m[9]) / m[5];
        let down = (m[9] - 1.0) / m[5];
        Self::new(left, right, down, up).ok()
    }

    /// Horizontal extent of the frustum at unit distance.
    #[must_use]
    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    /// Vertical extent of the frustum at unit distance.
    #[must_use]
    pub fn height(&self) -> f32 {
        self.up - self.down
    }

    /// The reverse-z, infinite-far, right-handed projection for this frustum: view-space points
    /// inside it map to NDC x and y in [-1, 1], and depth is `near / distance`.
    ///
    /// `near` must be positive and finite; anything else falls back to 0.01 so a bad value cannot
    /// produce a degenerate matrix.
    #[must_use]
    pub fn projection_reverse_z(&self, near: f32) -> Mat4 {
        let near = if near.is_finite() && near > 0.0 {
            near
        } else {
            0.01
        };
        let width = self.width();
        let height = self.height();
        Mat4::from_cols(
            Vec4::new(2.0 / width, 0.0, 0.0, 0.0),
            Vec4::new(0.0, 2.0 / height, 0.0, 0.0),
            Vec4::new(
                (self.right + self.left) / width,
                (self.up + self.down) / height,
                0.0,
                -1.0,
            ),
            Vec4::new(0.0, 0.0, near, 0.0),
        )
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec3;
    use proptest::prelude::*;

    use super::*;

    fn project(m: &Mat4, p: Vec3) -> Vec3 {
        let clip = *m * p.extend(1.0);
        clip.truncate() / clip.w
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(
            FovTangents::new(f32::NAN, 1.0, -1.0, 1.0),
            Err(FovError::NonFinite)
        );
        assert_eq!(
            FovTangents::new(1.0, -1.0, -1.0, 1.0),
            Err(FovError::Unordered)
        );
        assert_eq!(
            FovTangents::new(-1.0, 1.0, 1.0, 1.0),
            Err(FovError::Unordered)
        );
        assert!(FovTangents::symmetric(0.0, 1.0).is_err());
        assert!(FovTangents::symmetric(1.0, 4.0).is_err());
    }

    #[test]
    fn edges_map_to_ndc_edges_and_depth_is_reverse_z() {
        let fov = FovTangents::new(-0.8, 1.2, -0.7, 0.9).unwrap();
        let m = fov.projection_reverse_z(0.1);
        // A point on the right edge, two metres away.
        let on_right = project(&m, Vec3::new(1.2 * 2.0, 0.0, -2.0));
        assert!((on_right.x - 1.0).abs() < 1e-5, "{on_right:?}");
        let on_left = project(&m, Vec3::new(-0.8 * 2.0, 0.0, -2.0));
        assert!((on_left.x + 1.0).abs() < 1e-5, "{on_left:?}");
        let on_top = project(&m, Vec3::new(0.0, 0.9 * 2.0, -2.0));
        assert!((on_top.y - 1.0).abs() < 1e-5, "{on_top:?}");
        let on_bottom = project(&m, Vec3::new(0.0, -0.7 * 2.0, -2.0));
        assert!((on_bottom.y + 1.0).abs() < 1e-5, "{on_bottom:?}");
        // Depth: 1 at the near plane, smaller farther away, approaching 0.
        let near = project(&m, Vec3::new(0.0, 0.0, -0.1));
        let far = project(&m, Vec3::new(0.0, 0.0, -1000.0));
        assert!((near.z - 1.0).abs() < 1e-5);
        assert!(far.z > 0.0 && far.z < 1e-3);
    }

    #[test]
    fn symmetric_fov_matches_the_textbook_matrix() {
        let fov =
            FovTangents::symmetric(core::f32::consts::FRAC_PI_2, core::f32::consts::FRAC_PI_2)
                .unwrap();
        let m = fov.projection_reverse_z(0.5);
        let x = Vec4::new(1.0, 0.0, 0.0, 0.0);
        let y = Vec4::new(0.0, 1.0, 0.0, 0.0);
        assert!((m.x_axis - x).abs().max_element() < 1e-6);
        assert!((m.y_axis - y).abs().max_element() < 1e-6);
        assert!(
            (m.z_axis - Vec4::new(0.0, 0.0, 0.0, -1.0))
                .abs()
                .max_element()
                < 1e-6
        );
        assert!(
            (m.w_axis - Vec4::new(0.0, 0.0, 0.5, 0.0))
                .abs()
                .max_element()
                < 1e-6
        );
    }

    /// An OpenGL perspective matrix (depth -1..1), column-major, built from edge tangents.
    fn gl_projection(fov: &FovTangents, near: f32, far: f32) -> [f32; 16] {
        let (left, right) = (fov.left * near, fov.right * near);
        let (down, up) = (fov.down * near, fov.up * near);
        let mut m = [0.0; 16];
        m[0] = 2.0 * near / (right - left);
        m[5] = 2.0 * near / (up - down);
        m[8] = (right + left) / (right - left);
        m[9] = (up + down) / (up - down);
        m[10] = -(far + near) / (far - near);
        m[11] = -1.0;
        m[14] = -2.0 * far * near / (far - near);
        m
    }

    #[test]
    fn recovers_edges_from_a_gl_projection() {
        let fov = FovTangents::new(-1.1, 0.9, -1.0, 1.2).unwrap();
        let back = FovTangents::from_gl_projection(&gl_projection(&fov, 0.05, 100.0)).unwrap();
        for (a, b) in [
            (fov.left, back.left),
            (fov.right, back.right),
            (fov.down, back.down),
            (fov.up, back.up),
        ] {
            assert!((a - b).abs() < 1e-4);
        }
    }

    #[test]
    fn rejects_non_perspective_matrices() {
        assert!(FovTangents::from_gl_projection(&[0.0; 16]).is_none());
        assert!(FovTangents::from_gl_projection(&Mat4::IDENTITY.to_cols_array()).is_none());
    }

    #[test]
    fn bad_near_falls_back_instead_of_breaking() {
        let fov = FovTangents::symmetric(1.5, 1.5).unwrap();
        for near in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(
                fov.projection_reverse_z(near)
                    .to_cols_array()
                    .iter()
                    .all(|v| v.is_finite())
            );
        }
    }

    proptest! {
        #[test]
        fn gl_round_trip_holds_for_any_valid_frustum(
            l in -2.0_f32..-0.2, r in 0.2_f32..2.0, d in -2.0_f32..-0.2, u in 0.2_f32..2.0,
            near in 0.01_f32..1.0,
        ) {
            let fov = FovTangents::new(l, r, d, u).unwrap();
            let back = FovTangents::from_gl_projection(&gl_projection(&fov, near, 500.0)).unwrap();
            prop_assert!((fov.left - back.left).abs() < 1e-3);
            prop_assert!((fov.right - back.right).abs() < 1e-3);
            prop_assert!((fov.down - back.down).abs() < 1e-3);
            prop_assert!((fov.up - back.up).abs() < 1e-3);
        }
    }
}
