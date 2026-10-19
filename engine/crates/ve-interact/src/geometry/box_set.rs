//! Picking one of many copies of the same box (a forest of trees, a field of rocks) without testing every copy exactly: a world sphere
//! per copy rejects most of them with a few multiplications, and only the survivors get the exact affine-box test.
use super::{Ray, ray_affine_box};
use glam::{Mat4, Vec3};
use std::sync::Arc;

/// Copies of one local box, each placed by its own matrix.
#[derive(Clone, Debug)]
pub struct BoxSet {
    transforms: Arc<[Mat4]>,
    /// Centre and radius of a sphere that holds each copy.
    spheres: Arc<[(Vec3, f32)]>,
    min: Vec3,
    max: Vec3,
}

impl BoxSet {
    /// The copies of the box `min..max` placed by `transforms`.
    #[must_use]
    pub fn new(transforms: Arc<[Mat4]>, min: Vec3, max: Vec3) -> Self {
        let centre = (min + max) * 0.5;
        let half_diagonal = (max - min).length() * 0.5;
        let spheres = transforms
            .iter()
            .map(|matrix| {
                // The Frobenius norm of the linear part bounds how much the matrix can stretch anything (shear included).
                let stretch = (matrix.x_axis.truncate().length_squared()
                    + matrix.y_axis.truncate().length_squared()
                    + matrix.z_axis.truncate().length_squared())
                .sqrt();
                (matrix.transform_point3(centre), half_diagonal * stretch)
            })
            .collect();
        Self {
            transforms,
            spheres,
            min,
            max,
        }
    }

    /// How many copies.
    #[must_use]
    pub fn len(&self) -> usize {
        self.transforms.len()
    }

    /// Whether there are no copies.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.transforms.is_empty()
    }

    /// The box around every valid (finite) copy, or `None` when there is none.
    #[must_use]
    pub fn world_bounds(&self) -> Option<(Vec3, Vec3)> {
        let corners = |i: usize| {
            let (low, high) = (self.min, self.max);
            (0..8)
                .map(move |c| {
                    Vec3::new(
                        if c & 1 == 0 { low.x } else { high.x },
                        if c & 2 == 0 { low.y } else { high.y },
                        if c & 4 == 0 { low.z } else { high.z },
                    )
                })
                .map(move |corner| self.transforms[i].transform_point3(corner))
        };
        let mut bounds: Option<(Vec3, Vec3)> = None;
        for i in 0..self.transforms.len() {
            if !self.transforms[i].is_finite() {
                continue;
            }
            for point in corners(i) {
                bounds = Some(match bounds {
                    Some((low, high)) => (low.min(point), high.max(point)),
                    None => (point, point),
                });
            }
        }
        bounds
    }

    /// The distance along `ray` to the nearest copy it hits, and which copy that is.
    #[must_use]
    pub fn pick(&self, ray: &Ray) -> Option<(f32, usize)> {
        let mut best: Option<(f32, usize)> = None;
        for (index, (centre, radius)) in self.spheres.iter().enumerate() {
            if !ray_touches_sphere(ray, *centre, *radius) {
                continue;
            }
            if let Some(distance) = ray_affine_box(ray, self.min, self.max, self.transforms[index])
                && best.is_none_or(|(nearest, _)| distance < nearest)
            {
                best = Some((distance, index));
            }
        }
        best
    }
}

/// Whether a ray might touch a sphere: it passes within the radius of the centre, with the centre not wholly behind the origin.
fn ray_touches_sphere(ray: &Ray, centre: Vec3, radius: f32) -> bool {
    if !centre.is_finite() || !radius.is_finite() {
        return false;
    }
    let to_centre = centre - ray.origin;
    let along = to_centre.dot(ray.direction);
    if along + radius < 0.0 {
        return false;
    }
    to_centre.length_squared() - along * along <= radius * radius
}

#[cfg(test)]
#[path = "box_set_tests.rs"]
mod tests;
