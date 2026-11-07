//! Scene queries exposed without Rapier types.
use parry3d::query::ShapeCastOptions;
use rapier3d::na::{Point3, Vector3};
use rapier3d::prelude::{Collider, ColliderHandle, QueryFilter, Ray};

use crate::{BodyId, Shape, World};

/// The first body hit by a ray.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    /// The body that was hit.
    pub body: BodyId,
    /// The world-space point of impact.
    pub point: [f32; 3],
    /// The world-space surface normal.
    pub normal: [f32; 3],
    /// Distance from the ray origin in metres.
    pub distance: f32,
}

/// The surface a small sphere touches.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceHit {
    /// The body whose surface it is.
    pub body: BodyId,
    /// The unit direction out of the surface, the way the sphere should move.
    pub normal: [f32; 3],
    /// How far the sphere sinks into the surface, metres.
    pub depth: f32,
}

/// The first body touched by a swept shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeHit {
    /// The body that was hit.
    pub body: BodyId,
    /// The world-space point of impact on the body.
    pub point: [f32; 3],
    /// The world-space normal on the body.
    pub normal: [f32; 3],
    /// Distance travelled before impact in metres.
    pub distance: f32,
}

fn body_id(world: &World, collider: ColliderHandle) -> Option<BodyId> {
    world
        .entries
        .iter()
        .position(|entry| entry.live && entry.collider == collider)
        .map(BodyId)
}

fn unit_direction(direction: [f32; 3]) -> Option<Vector3<f32>> {
    if !direction.iter().all(|value| value.is_finite()) {
        return None;
    }
    let vector = Vector3::new(direction[0], direction[1], direction[2]);
    let length = vector.norm();
    if !length.is_finite() || length <= f32::EPSILON {
        return None;
    }
    Some(vector / length)
}

impl World {
    fn refresh_queries(&mut self) {
        if self.query_dirty {
            self.query.update(&self.colliders);
            self.query_dirty = false;
        }
    }

    /// Casts a ray and returns the nearest eligible body hit.
    #[must_use]
    pub fn raycast(
        &mut self,
        origin: [f32; 3],
        direction: [f32; 3],
        max_distance: f32,
        mask: u32,
    ) -> Option<RayHit> {
        self.raycast_from(origin, direction, max_distance, mask, None)
    }

    /// Like [`World::raycast`], ignoring one body (a vehicle's wheels must not hit their own chassis).
    #[must_use]
    pub fn raycast_excluding(
        &mut self,
        origin: [f32; 3],
        direction: [f32; 3],
        max_distance: f32,
        mask: u32,
        exclude: BodyId,
    ) -> Option<RayHit> {
        let handle = self.entry_handle(exclude).ok();
        self.raycast_from(origin, direction, max_distance, mask, handle)
    }

    /// A body's mass in kilograms (0 for a static body).
    ///
    /// # Errors
    /// [`crate::Error::NoSuchBody`].
    pub fn body_mass(&self, id: BodyId) -> Result<f32, crate::Error> {
        Ok(self.bodies[self.entry_handle(id)?].mass())
    }

    /// The velocity of the point `point` (world coordinates) of a body, including the part from its spin.
    ///
    /// # Errors
    /// [`crate::Error::NoSuchBody`].
    pub fn point_velocity(&self, id: BodyId, point: [f32; 3]) -> Result<[f32; 3], crate::Error> {
        let body = &self.bodies[self.entry_handle(id)?];
        if body.is_sleeping() {
            return Ok([0.0; 3]);
        }
        let v = body.velocity_at_point(&Point3::from(point));
        Ok([v.x, v.y, v.z])
    }

    /// Applies an impulse (kilogram metres per second) to a body at a world point, waking it.
    ///
    /// # Errors
    /// [`crate::Error::NoSuchBody`], or [`crate::Error::BadValue`] for numbers that are not finite.
    pub fn apply_impulse_at(
        &mut self,
        id: BodyId,
        impulse: [f32; 3],
        point: [f32; 3],
    ) -> Result<(), crate::Error> {
        if !impulse.iter().chain(&point).all(|value| value.is_finite()) {
            return Err(crate::Error::BadValue("an impulse must be finite"));
        }
        let handle = self.entry_handle(id)?;
        self.bodies[handle].apply_impulse_at_point(
            Vector3::new(impulse[0], impulse[1], impulse[2]),
            Point3::from(point),
            true,
        );
        Ok(())
    }

    fn raycast_from(
        &mut self,
        origin: [f32; 3],
        direction: [f32; 3],
        max_distance: f32,
        mask: u32,
        exclude: Option<rapier3d::prelude::RigidBodyHandle>,
    ) -> Option<RayHit> {
        if !origin.iter().all(|value| value.is_finite())
            || !max_distance.is_finite()
            || max_distance < 0.0
        {
            return None;
        }
        let direction = unit_direction(direction)?;
        self.refresh_queries();
        let ray = Ray::new(Point3::from(origin), direction);
        let membership_matches = |_: ColliderHandle, collider: &Collider| {
            collider.collision_groups().memberships.bits() & mask != 0
        };
        let filter = QueryFilter {
            predicate: Some(&membership_matches),
            exclude_rigid_body: exclude,
            ..QueryFilter::default()
        };
        let (collider, intersection) = self.query.cast_ray_and_get_normal(
            &self.bodies,
            &self.colliders,
            &ray,
            max_distance,
            true,
            filter,
        )?;
        let body = body_id(self, collider)?;
        let distance = intersection.time_of_impact;
        let point = Point3::from(origin) + direction * distance;
        Some(RayHit {
            body,
            point: point.into(),
            normal: intersection.normal.into(),
            distance,
        })
    }

    /// The surface a small sphere is touching, if it is: how far it sinks into it and the way out. Works for a point inside a solid too (pushed out the
    /// short way). A sphere of radius 0 is touching only when inside. `None` when nothing is within the radius, or the numbers are not finite.
    #[must_use]
    pub fn nearest_surface(&mut self, centre: [f32; 3], radius: f32, mask: u32) -> Option<SurfaceHit> {
        if !centre.iter().all(|v| v.is_finite()) || !radius.is_finite() || radius < 0.0 {
            return None;
        }
        self.refresh_queries();
        let membership_matches = |_: ColliderHandle, collider: &Collider| {
            collider.collision_groups().memberships.bits() & mask != 0
        };
        let filter = QueryFilter {
            predicate: Some(&membership_matches),
            ..QueryFilter::default()
        };
        let at = Point3::from(centre);
        let (collider, projection) = self
            .query
            .project_point(&self.bodies, &self.colliders, &at, false, filter)?;
        let to_surface = projection.point - at;
        let distance = to_surface.norm();
        let (depth, direction) = if projection.is_inside {
            // The surface is the way out: the sphere is `distance` deep plus its own radius.
            (distance + radius, to_surface)
        } else {
            (radius - distance, -to_surface)
        };
        if depth <= 0.0 || (!projection.is_inside && distance > radius) {
            return None;
        }
        let length = direction.norm();
        if length < 1.0e-9 {
            return None;
        }
        Some(SurfaceHit {
            body: body_id(self, collider)?,
            normal: (direction / length).into(),
            depth,
        })
    }

    /// Returns bodies intersecting a sphere, ordered by body id.
    #[must_use]
    pub fn overlap_sphere(&mut self, centre: [f32; 3], radius: f32, mask: u32) -> Vec<BodyId> {
        if !centre.iter().all(|value| value.is_finite()) || !radius.is_finite() || radius <= 0.0 {
            return Vec::new();
        }
        self.refresh_queries();
        let shape = rapier3d::prelude::SharedShape::ball(radius);
        let position = rapier3d::na::Isometry3::translation(centre[0], centre[1], centre[2]);
        let membership_matches = |_: ColliderHandle, collider: &Collider| {
            collider.collision_groups().memberships.bits() & mask != 0
        };
        let filter = QueryFilter {
            predicate: Some(&membership_matches),
            ..QueryFilter::default()
        };
        let mut found = Vec::new();
        self.query.intersections_with_shape(
            &self.bodies,
            &self.colliders,
            &position,
            shape.as_ref(),
            filter,
            |collider| {
                if let Some(id) = body_id(self, collider) {
                    found.push(id);
                }
                true
            },
        );
        found.sort_unstable();
        found.dedup();
        found
    }

    /// Sweeps a shape in a direction and returns the first eligible body hit.
    #[must_use]
    pub fn shape_cast(
        &mut self,
        shape: &Shape,
        from: [f32; 3],
        direction: [f32; 3],
        max_distance: f32,
        mask: u32,
    ) -> Option<ShapeHit> {
        if !from.iter().all(|value| value.is_finite())
            || !max_distance.is_finite()
            || max_distance < 0.0
        {
            return None;
        }
        let direction = unit_direction(direction)?;
        let collider = shape.collider().ok()?.build();
        let shape = collider.shared_shape();
        self.refresh_queries();
        let position = rapier3d::na::Isometry3::translation(from[0], from[1], from[2]);
        let options = ShapeCastOptions::with_max_time_of_impact(max_distance);
        let membership_matches = |_: ColliderHandle, collider: &Collider| {
            collider.collision_groups().memberships.bits() & mask != 0
        };
        let filter = QueryFilter {
            predicate: Some(&membership_matches),
            ..QueryFilter::default()
        };
        let (collider, hit) = self.query.cast_shape(
            &self.bodies,
            &self.colliders,
            &position,
            &direction,
            shape.as_ref(),
            options,
            filter,
        )?;
        Some(ShapeHit {
            body: body_id(self, collider)?,
            point: hit.witness1.into(),
            normal: hit.normal1.into_inner().into(),
            distance: hit.time_of_impact,
        })
    }

    /// Returns the bodies currently intersecting a sensor, ordered by body id.
    ///
    /// # Errors
    /// Returns [`crate::Error::NoSuchBody`] when `sensor` does not name a body.
    pub fn touching(&mut self, sensor: BodyId) -> Result<Vec<BodyId>, crate::Error> {
        let collider = self.entry(sensor)?.collider;
        if !self.colliders[collider].is_sensor() {
            return Ok(Vec::new());
        }
        let mut found = self
            .narrow_phase
            .intersection_pairs()
            .filter(|(a, b, intersecting)| *intersecting && (*a == collider || *b == collider))
            .filter_map(|(a, b, _)| body_id(self, if a == collider { b } else { a }))
            .collect::<Vec<_>>();
        found.sort_unstable();
        found.dedup();
        Ok(found)
    }

    /// Sets the next pose of a kinematic body so Rapier derives its velocity for the next step.
    ///
    /// # Errors
    /// Returns [`crate::Error::NoSuchBody`] for an unknown id or [`crate::Error::BadValue`] for a non-kinematic body or
    /// a non-finite pose.
    pub fn move_kinematic(
        &mut self,
        id: BodyId,
        position: [f32; 3],
        rotation: [f32; 4],
    ) -> Result<(), crate::Error> {
        if !position
            .iter()
            .chain(&rotation)
            .all(|value| value.is_finite())
        {
            return Err(crate::Error::BadValue("a pose must be finite"));
        }
        let handle = self.entry(id)?.handle;
        if self.bodies[handle].body_type()
            != rapier3d::prelude::RigidBodyType::KinematicPositionBased
        {
            return Err(crate::Error::BadValue("only kinematic bodies can be moved"));
        }
        self.bodies[handle].set_next_kinematic_position(super::isometry(position, rotation));
        self.query_dirty = true;
        Ok(())
    }
}
