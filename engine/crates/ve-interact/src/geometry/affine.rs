//! Selection-only affine boxes: transform the ray, preserving its world distance.
use super::{Ray, ray_aabb};
use glam::{Mat4, Vec3};

/// Ray distance to an affine box, including shear. Invalid/collapsed/mirrored matrices are refused.
#[must_use]
pub fn ray_affine_box(ray: &Ray, min: Vec3, max: Vec3, matrix: Mat4) -> Option<f32> {
    if !matrix.is_finite()
        || matrix.x_axis.w.abs() > 1e-6
        || matrix.y_axis.w.abs() > 1e-6
        || matrix.z_axis.w.abs() > 1e-6
        || (matrix.w_axis.w - 1.0).abs() > 1e-6
        || matrix.determinant() <= 1e-12
        || !min.is_finite()
        || !max.is_finite()
        || !min.cmple(max).all()
    {
        return None;
    }
    let inverse = matrix.inverse();
    let origin = inverse.transform_point3(ray.origin);
    let direction = inverse.transform_vector3(ray.direction);
    let scale = direction.length();
    let local = Ray::new(origin, direction)?;
    ray_aabb(&local, min, max).map(|distance| distance / scale)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn g4c_affine_picking_keeps_world_distance_and_handles_shear() {
        let mut matrix = Mat4::from_translation(Vec3::new(0.0, 0.0, -5.0));
        matrix.y_axis.x = 0.4;
        let ray = Ray::new(Vec3::ZERO, Vec3::NEG_Z).unwrap();
        assert!(
            (ray_affine_box(&ray, Vec3::splat(-0.5), Vec3::splat(0.5), matrix).unwrap() - 4.5)
                .abs()
                < 1e-5
        );
        assert!(ray_affine_box(&ray, Vec3::ZERO, Vec3::ONE, Mat4::ZERO).is_none());
        assert!(
            ray_affine_box(
                &ray,
                Vec3::ZERO,
                Vec3::ONE,
                Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0))
            )
            .is_none()
        );
    }
}
