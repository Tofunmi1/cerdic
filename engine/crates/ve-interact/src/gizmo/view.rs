//! Camera projection math used to put gizmo handles under a desktop pointer.

use glam::{Quat, Vec2, Vec3};

use crate::Ray;

/// A perspective camera snapshot in world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraView {
    /// Camera position.
    pub position: Vec3,
    /// Camera orientation.
    pub rotation: Quat,
    /// Vertical field of view in radians.
    pub vertical_fov: f32,
}

impl CameraView {
    /// Builds a world-space pointer ray from normalized device coordinates (`-1..=1`).
    #[must_use]
    pub fn ray(self, ndc: Vec2, aspect: f32) -> Option<Ray> {
        if !self.valid(aspect) || !ndc.is_finite() {
            return None;
        }
        let tangent = (self.vertical_fov * 0.5).tan();
        let local = Vec3::new(ndc.x * aspect * tangent, ndc.y * tangent, -1.0);
        Ray::new(self.position, self.rotation * local)
    }

    /// Projects a world point to normalized device coordinates and positive camera depth.
    #[must_use]
    pub fn project(self, point: Vec3, aspect: f32) -> Option<Vec3> {
        if !self.valid(aspect) || !point.is_finite() {
            return None;
        }
        let camera = self.rotation.inverse() * (point - self.position);
        let depth = -camera.z;
        if depth <= 1.0e-6 {
            return None;
        }
        let tangent = (self.vertical_fov * 0.5).tan();
        Some(Vec3::new(
            camera.x / (depth * aspect * tangent),
            camera.y / (depth * tangent),
            depth,
        ))
    }

    fn valid(self, aspect: f32) -> bool {
        self.position.is_finite()
            && self.rotation.is_finite()
            && self.rotation.length_squared() > 1.0e-8
            && self.vertical_fov.is_finite()
            && self.vertical_fov > 1.0e-4
            && self.vertical_fov < std::f32::consts::PI - 1.0e-4
            && aspect.is_finite()
            && aspect > 1.0e-6
    }
}
