//! Constrained axis, plane and ring dragging, with deterministic world-space snapping.

use glam::Vec3;

use crate::Ray;

/// Which document-space quantity the drag changes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    /// Move along an axis or within a plane.
    Translate,
    /// Turn around a ring's axis.
    Rotate,
    /// Uniform scale along the chosen axis.
    Scale,
}

/// A geometric constraint in world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Constraint {
    /// Closest point between the pointer ray and an axis line.
    Axis {
        /// A point on the axis.
        origin: Vec3,
        /// Nonzero axis direction; normalized when the drag starts.
        direction: Vec3,
    },
    /// Intersection with a plane.
    Plane {
        /// A point on the plane.
        origin: Vec3,
        /// Nonzero plane normal; normalized when the drag starts.
        normal: Vec3,
    },
    /// Angular motion on a plane through the pivot.
    Ring {
        /// The pivot at the centre of the ring.
        center: Vec3,
        /// Nonzero ring axis; normalized when the drag starts.
        normal: Vec3,
    },
}

/// Optional positive snapping increments. Invalid values behave as disabled snapping.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Snap {
    /// Metres between translation steps.
    pub grid: Option<f32>,
    /// Radians between rotation steps.
    pub angle: Option<f32>,
    /// Uniform scale factor step.
    pub scale: Option<f32>,
}

/// A delta to apply to authored transform parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Delta {
    /// World-space translation from the drag start.
    Translation(Vec3),
    /// Signed turn around a unit world-space axis.
    Rotation {
        /// Unit world-space rotation axis.
        axis: Vec3,
        /// Signed rotation from the drag start, in radians.
        radians: f32,
    },
    /// Positive factor along the selected world axis; other axes remain unchanged.
    Scale {
        /// Unit world-space direction of the handle.
        axis: Vec3,
        /// Positive uniform factor from the drag start.
        factor: f32,
    },
}

/// A drag in progress. Repeated `update` calls accumulate ring turns across angle wraparound.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GizmoDrag {
    mode: Mode,
    constraint: Constraint,
    start: Vec3,
    previous_ring: Vec3,
    accumulated_angle: f32,
}

impl GizmoDrag {
    /// Starts when the ray gives an unambiguous point on the requested constraint.
    #[must_use]
    pub fn begin(mode: Mode, constraint: Constraint, ray: &Ray) -> Option<Self> {
        let start = sample(constraint, ray)?;
        Some(Self {
            mode,
            constraint,
            start,
            previous_ring: start,
            accumulated_angle: 0.0,
        })
    }

    /// Updates the drag; `None` means the ray is parallel or otherwise degenerate this frame.
    pub fn update(&mut self, ray: &Ray, snap: Snap) -> Option<Delta> {
        let current = sample(self.constraint, ray)?;
        match self.mode {
            Mode::Translate => self.translation(current, snap),
            Mode::Scale => self.scaling(current, snap),
            Mode::Rotate => self.rotation(current, snap),
        }
    }

    fn translation(&self, current: Vec3, snap: Snap) -> Option<Delta> {
        let delta = match self.constraint {
            Constraint::Axis { direction, .. } => {
                let axis = direction.normalize();
                axis * snap_scalar((current - self.start).dot(axis), snap.grid)
            }
            Constraint::Plane { .. } => snap_vector(current - self.start, snap.grid),
            Constraint::Ring { .. } => return None,
        };
        delta.is_finite().then_some(Delta::Translation(delta))
    }

    fn scaling(&self, current: Vec3, snap: Snap) -> Option<Delta> {
        let Constraint::Axis { direction, .. } = self.constraint else {
            return None;
        };
        let axis = direction.normalize();
        let distance = (current - self.start).dot(axis);
        let raw = (1.0 + distance).max(1.0e-4);
        let factor = snap_scalar(raw, snap.scale).max(1.0e-4);
        factor.is_finite().then_some(Delta::Scale { axis, factor })
    }

    fn rotation(&mut self, current: Vec3, snap: Snap) -> Option<Delta> {
        let Constraint::Ring { normal, .. } = self.constraint else {
            return None;
        };
        let from = (self.previous_ring - ring_center(self.constraint)).try_normalize()?;
        let to = (current - ring_center(self.constraint)).try_normalize()?;
        let step = normal.dot(from.cross(to)).atan2(from.dot(to));
        if !step.is_finite() {
            return None;
        }
        self.accumulated_angle += step;
        self.previous_ring = current;
        let radians = snap_scalar(self.accumulated_angle, snap.angle);
        Some(Delta::Rotation {
            axis: normal.normalize(),
            radians,
        })
    }
}

fn ring_center(constraint: Constraint) -> Vec3 {
    match constraint {
        Constraint::Ring { center, .. } => center,
        Constraint::Axis { origin, .. } | Constraint::Plane { origin, .. } => origin,
    }
}

fn sample(constraint: Constraint, ray: &Ray) -> Option<Vec3> {
    match constraint {
        Constraint::Axis { origin, direction } => closest_axis_point(ray, origin, direction),
        Constraint::Plane { origin, normal } => plane_hit(ray, origin, normal),
        Constraint::Ring { center, normal } => plane_hit(ray, center, normal),
    }
}

fn closest_axis_point(ray: &Ray, origin: Vec3, direction: Vec3) -> Option<Vec3> {
    let axis = direction.try_normalize()?;
    let cosine = ray.direction.dot(axis);
    let denominator = 1.0 - cosine * cosine;
    if denominator.abs() < 1.0e-6 {
        return None;
    }
    let offset = ray.origin - origin;
    let along_axis = (axis.dot(offset) - cosine * ray.direction.dot(offset)) / denominator;
    let along_ray = cosine * along_axis - ray.direction.dot(offset);
    (along_ray >= 0.0).then_some(origin + axis * along_axis)
}

fn plane_hit(ray: &Ray, origin: Vec3, normal: Vec3) -> Option<Vec3> {
    let normal = normal.try_normalize()?;
    let denominator = ray.direction.dot(normal);
    if denominator.abs() < 1.0e-6 {
        return None;
    }
    let distance = (origin - ray.origin).dot(normal) / denominator;
    (distance >= 0.0).then(|| ray.at(distance))
}

fn snap_scalar(value: f32, increment: Option<f32>) -> f32 {
    match increment.filter(|step| step.is_finite() && *step > 1.0e-6) {
        Some(step) => (value / step).round() * step,
        None => value,
    }
}

fn snap_vector(value: Vec3, increment: Option<f32>) -> Vec3 {
    Vec3::new(
        snap_scalar(value.x, increment),
        snap_scalar(value.y, increment),
        snap_scalar(value.z, increment),
    )
}
