//! Rays and the shapes they pick.

use glam::{Mat4, Quat, Vec3};

use crate::pose::{Decomposed, decompose};

/// A half-line: `origin + direction * t` for `t >= 0`. The direction is always unit length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    /// Where the ray starts.
    pub origin: Vec3,
    /// Unit direction.
    pub direction: Vec3,
}

impl Ray {
    /// A ray, or `None` if the origin is not finite or the direction is zero or not finite.
    #[must_use]
    pub fn new(origin: Vec3, direction: Vec3) -> Option<Self> {
        if !origin.is_finite() || !direction.is_finite() {
            return None;
        }
        let direction = direction.try_normalize()?;
        Some(Self { origin, direction })
    }

    /// The point `distance` along the ray.
    #[must_use]
    pub fn at(&self, distance: f32) -> Vec3 {
        self.origin + self.direction * distance
    }
}

/// Something a ray can hit, in world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// A ball.
    Sphere {
        /// Centre.
        center: Vec3,
        /// Radius in metres.
        radius: f32,
    },
    /// A box with any orientation.
    Obb {
        /// Centre.
        center: Vec3,
        /// Half the size along each of the box's own axes.
        half_extents: Vec3,
        /// Orientation of the box's axes.
        rotation: Quat,
    },
}

impl Shape {
    /// The shape of a local-space box `[min, max]` placed by `world_from_local` (translation,
    /// rotation and any scale), as an oriented box.
    ///
    /// An oriented box cannot represent every transform, so this returns `None` instead of an
    /// approximation when the bounds are not finite or inverted, or the transform is not a
    /// rotation, a positive per-axis scale and a translation: that rules out collapsed axes,
    /// mirroring and shear (a rotated child under a non-uniformly scaled parent). A caller can hide
    /// such an object from picking or place it differently; it is never silently mis-picked.
    #[must_use]
    pub fn from_local_bounds(min: Vec3, max: Vec3, world_from_local: Mat4) -> Option<Self> {
        if !(min.is_finite() && max.is_finite() && min.cmple(max).all()) {
            return None;
        }
        let Decomposed {
            scale,
            rotation,
            translation,
        } = decompose(world_from_local)?;
        let local_center = (min + max) * 0.5;
        Some(Self::Obb {
            center: translation + rotation * (scale * local_center),
            half_extents: (max - min) * 0.5 * scale,
            rotation,
        })
    }

    /// Distance along `ray` to the first point of the shape at or beyond the ray's origin, if the
    /// ray reaches it. A ray that starts inside the shape hits at distance zero.
    #[must_use]
    pub fn raycast(&self, ray: &Ray) -> Option<f32> {
        match *self {
            Self::Sphere { center, radius } => ray_sphere(ray, center, radius),
            Self::Obb {
                center,
                half_extents,
                rotation,
            } => {
                // Work in the box's own frame, where it is axis aligned.
                let inverse = rotation.inverse();
                let local = Ray {
                    origin: inverse * (ray.origin - center),
                    direction: inverse * ray.direction,
                };
                ray_aabb(&local, -half_extents, half_extents)
            }
        }
    }
}

fn ray_sphere(ray: &Ray, center: Vec3, radius: f32) -> Option<f32> {
    if radius.is_nan() || radius < 0.0 {
        return None;
    }
    // Form the perpendicular vector directly in double precision. Subtracting squared
    // distances loses the small radius when the ray starts far from the sphere.
    let to_center = center.as_dvec3() - ray.origin.as_dvec3();
    let direction = ray.direction.as_dvec3();
    let direction_squared = direction.length_squared();
    let along = to_center.dot(direction) / direction_squared;
    let off_axis_squared = (to_center - direction * along).length_squared();
    let radius_squared = f64::from(radius).powi(2);
    if off_axis_squared > radius_squared {
        return None;
    }
    let half_chord = ((radius_squared - off_axis_squared) / direction_squared).sqrt();
    let near = along - half_chord;
    let far = along + half_chord;
    if far < 0.0 {
        None
    } else {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the picking API returns f32 distances; intermediates avoid cancellation in f64"
        )]
        let distance = near.max(0.0) as f32;
        Some(distance)
    }
}

/// The slab method: intersect the ray with the three pairs of parallel planes.
fn ray_aabb(ray: &Ray, min: Vec3, max: Vec3) -> Option<f32> {
    let mut enter = 0.0_f32;
    let mut exit = f32::INFINITY;
    for axis in 0..3 {
        let origin = ray.origin[axis];
        let direction = ray.direction[axis];
        if direction.abs() < 1e-9 {
            // Parallel to this pair of planes: it either stays inside the slab or never enters.
            if origin < min[axis] || origin > max[axis] {
                return None;
            }
            continue;
        }
        let a = (min[axis] - origin) / direction;
        let b = (max[axis] - origin) / direction;
        let (near, far) = if a < b { (a, b) } else { (b, a) };
        enter = enter.max(near);
        exit = exit.min(far);
        if enter > exit {
            return None;
        }
    }
    Some(enter)
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn ray(origin: Vec3, direction: Vec3) -> Ray {
        Ray::new(origin, direction).expect("test rays are valid")
    }

    #[test]
    fn rays_reject_bad_input_and_normalise_directions() {
        assert!(Ray::new(Vec3::ZERO, Vec3::ZERO).is_none());
        assert!(Ray::new(Vec3::NAN, Vec3::X).is_none());
        assert!(Ray::new(Vec3::ZERO, Vec3::new(f32::INFINITY, 0.0, 0.0)).is_none());
        let r = ray(Vec3::ZERO, Vec3::new(0.0, 0.0, -5.0));
        assert!((r.direction.length() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_sphere_is_hit_at_its_near_surface() {
        let sphere = Shape::Sphere {
            center: Vec3::new(0.0, 0.0, -5.0),
            radius: 1.0,
        };
        let distance = sphere.raycast(&ray(Vec3::ZERO, Vec3::NEG_Z)).unwrap();
        assert!((distance - 4.0).abs() < 1e-5);
        assert!(
            sphere.raycast(&ray(Vec3::ZERO, Vec3::Z)).is_none(),
            "behind the ray"
        );
        assert!(
            sphere
                .raycast(&ray(Vec3::new(2.0, 0.0, 0.0), Vec3::NEG_Z))
                .is_none(),
            "beside it"
        );
    }

    #[test]
    fn a_ray_starting_inside_hits_at_zero() {
        let sphere = Shape::Sphere {
            center: Vec3::ZERO,
            radius: 2.0,
        };
        assert_eq!(
            sphere.raycast(&ray(Vec3::new(0.5, 0.0, 0.0), Vec3::X)),
            Some(0.0)
        );
        let cube =
            Shape::from_local_bounds(Vec3::splat(-1.0), Vec3::splat(1.0), Mat4::IDENTITY).unwrap();
        assert_eq!(cube.raycast(&ray(Vec3::ZERO, Vec3::Y)), Some(0.0));
    }

    #[test]
    fn a_box_is_hit_on_its_front_face_even_when_rotated_and_scaled() {
        let placed = Mat4::from_scale_rotation_translation(
            Vec3::new(2.0, 1.0, 1.0),
            Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            Vec3::new(0.0, 0.0, -10.0),
        );
        // A 1 m cube scaled 2x along x, then turned a quarter about y: it is now 2 m deep along z.
        let shape = Shape::from_local_bounds(Vec3::splat(-0.5), Vec3::splat(0.5), placed).unwrap();
        let distance = shape.raycast(&ray(Vec3::ZERO, Vec3::NEG_Z)).unwrap();
        assert!((distance - 9.0).abs() < 1e-4, "{distance}");
    }

    #[test]
    fn transforms_a_box_cannot_represent_give_no_shape() {
        let unit = (Vec3::splat(-0.5), Vec3::splat(0.5));
        let shape = |m: Mat4| Shape::from_local_bounds(unit.0, unit.1, m);
        assert!(shape(Mat4::from_scale(Vec3::ZERO)).is_none(), "collapsed");
        assert!(
            shape(Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0))).is_none(),
            "mirrored"
        );
        let sheared = Mat4::from_scale(Vec3::new(3.0, 1.0, 1.0)) * Mat4::from_rotation_z(0.6);
        assert!(
            shape(sheared).is_none(),
            "sheared by a rotated child under a stretched parent"
        );
        assert!(shape(Mat4::from_cols_array(&[f32::NAN; 16])).is_none());
        // Bad bounds: not finite, or inverted.
        assert!(Shape::from_local_bounds(Vec3::splat(f32::NAN), unit.1, Mat4::IDENTITY).is_none());
        assert!(
            Shape::from_local_bounds(unit.1, unit.0, Mat4::IDENTITY).is_none(),
            "min > max"
        );
        assert!(
            Shape::from_local_bounds(unit.0, Vec3::splat(f32::INFINITY), Mat4::IDENTITY).is_none()
        );
    }

    #[test]
    fn a_ray_almost_parallel_to_a_face_gives_a_finite_answer() {
        let cube =
            Shape::from_local_bounds(Vec3::splat(-1.0), Vec3::splat(1.0), Mat4::IDENTITY).unwrap();
        // Skimming just above the top face, then just below it, at a tiny slope.
        for slope in [1.0e-7_f32, -1.0e-7, 1.0e-12] {
            let direction = Vec3::new(1.0, slope, 0.0).normalize();
            let above = cube.raycast(&ray(Vec3::new(-5.0, 1.0 + 1.0e-3, 0.0), direction));
            let inside = cube.raycast(&ray(Vec3::new(-5.0, 0.999, 0.0), direction));
            assert!(above.is_none_or(f32::is_finite));
            assert!(inside.is_some_and(f32::is_finite), "slope {slope}");
        }
    }

    #[test]
    fn rays_parallel_to_a_face_miss_or_graze_correctly() {
        let cube =
            Shape::from_local_bounds(Vec3::splat(-1.0), Vec3::splat(1.0), Mat4::IDENTITY).unwrap();
        assert!(
            cube.raycast(&ray(Vec3::new(-5.0, 2.0, 0.0), Vec3::X))
                .is_none()
        );
        assert!(
            cube.raycast(&ray(Vec3::new(-5.0, 0.5, 0.0), Vec3::X))
                .is_some()
        );
    }

    proptest! {
        #[test]
        fn a_ray_aimed_at_a_sphere_hits_it_at_the_right_distance(
            cx in -5.0_f32..5.0, cy in -5.0_f32..5.0, cz in -5.0_f32..5.0,
            radius in 0.1_f32..3.0,
            ox in -20.0_f32..20.0, oy in -20.0_f32..20.0, oz in -20.0_f32..20.0,
        ) {
            let center = Vec3::new(cx, cy, cz);
            let origin = Vec3::new(ox, oy, oz);
            prop_assume!(origin.distance(center) > radius + 0.01);
            let r = Ray::new(origin, center - origin).unwrap();
            let distance = Shape::Sphere { center, radius }.raycast(&r).unwrap();
            prop_assert!((r.at(distance).distance(center) - radius).abs() < 1e-3);
        }

        #[test]
        fn a_ray_aimed_at_a_boxes_centre_always_hits_it(
            hx in 0.1_f32..2.0, hy in 0.1_f32..2.0, hz in 0.1_f32..2.0,
            angle in 0.0_f32..std::f32::consts::TAU,
            ox in -20.0_f32..20.0, oy in -20.0_f32..20.0, oz in -20.0_f32..20.0,
        ) {
            let center = Vec3::new(1.0, 2.0, -3.0);
            let shape = Shape::Obb {
                center,
                half_extents: Vec3::new(hx, hy, hz),
                rotation: Quat::from_rotation_y(angle) * Quat::from_rotation_x(angle * 0.5),
            };
            let origin = Vec3::new(ox, oy, oz);
            prop_assume!(origin.distance(center) > 4.0);
            let r = Ray::new(origin, center - origin).unwrap();
            let distance = shape.raycast(&r);
            prop_assert!(distance.is_some());
            prop_assert!(distance.unwrap() <= origin.distance(center) + 1e-3);
        }
    }
}
