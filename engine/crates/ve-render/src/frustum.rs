//! View frustums for culling: six planes read straight from a clip-from-world matrix, and a sphere test.

use bevy_math::{Mat4, Vec3, Vec4};

/// The volume a view sees, as six inward-facing planes `(normal, distance)`: a point `p` is inside a plane when
/// `normal . p + distance >= 0`. A plane that the matrix leaves degenerate (an infinite far plane) is skipped.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Frustum {
    planes: [Option<Vec4>; 6],
}

impl Frustum {
    /// The frustum of a clip-from-world matrix in wgpu's clip space (`-w <= x, y <= w`, `0 <= z <= w`), whatever the depth
    /// direction: the planes are the same six inequalities, so reverse-Z and orthographic matrices work alike.
    pub(crate) fn from_clip_from_world(clip_from_world: Mat4) -> Self {
        let (r0, r1, r2, r3) = (
            clip_from_world.row(0),
            clip_from_world.row(1),
            clip_from_world.row(2),
            clip_from_world.row(3),
        );
        Self {
            planes: [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r2, r3 - r2].map(normalised),
        }
    }

    /// Only the four sides: for a shadow cascade, whose casters may lie in front of the slice it covers (between the sun
    /// and the view's volume), so the near and far planes must not cull them.
    pub(crate) fn sides_only(mut self) -> Self {
        self.planes[4] = None;
        self.planes[5] = None;
        self
    }

    /// Whether a sphere is at least partly inside. Conservative: it never rejects a sphere that is visible, and may keep one
    /// that lies just outside a corner.
    pub(crate) fn intersects_sphere(&self, centre: Vec3, radius: f32) -> bool {
        self.planes
            .iter()
            .flatten()
            .all(|plane| plane.truncate().dot(centre) + plane.w >= -radius)
    }
}

/// A plane with a unit normal, or `None` when its normal is (nearly) zero.
fn normalised(plane: Vec4) -> Option<Vec4> {
    let length = plane.truncate().length();
    (length > 1.0e-12).then(|| plane / length)
}

#[cfg(test)]
#[path = "frustum_tests.rs"]
mod tests;
