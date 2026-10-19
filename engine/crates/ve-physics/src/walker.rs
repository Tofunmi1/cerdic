//! A walking character: a capsule that moves over the static and dynamic bodies with gravity, slopes, small steps and jumping.
//!
//! The walker has no collider of its own in the world (nothing is pushed by it); each step it asks Rapier's kinematic character
//! controller how far the capsule may really move.
use rapier3d::control::{CharacterAutostep, CharacterLength, KinematicCharacterController};
use rapier3d::na::{Isometry3, Point3, Vector3};
use rapier3d::prelude::{QueryFilter, Ray, SharedShape};

use crate::{Error, STEP, World};

/// The capsule and where it starts.
#[derive(Clone, Copy, Debug)]
pub struct WalkerDesc {
    /// Where the capsule's centre starts, in metres.
    pub position: [f32; 3],
    /// Total height of the capsule, head to feet.
    pub height: f32,
    /// Radius of the capsule.
    pub radius: f32,
}

/// What the walker is doing after a step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WalkerState {
    /// Where the capsule's centre is now.
    pub position: [f32; 3],
    /// Whether the feet are on something.
    pub grounded: bool,
    /// Vertical speed in metres per second (negative while falling).
    pub vertical_speed: f32,
}

/// The walker inside a world.
pub(crate) struct Walker {
    shape: SharedShape,
    position: Vector3<f32>,
    vertical_speed: f32,
    grounded: bool,
    controller: KinematicCharacterController,
}

impl World {
    /// The height of the highest surface below `(x, from_y, z)` straight down, if there is one within 1,000 m.
    #[must_use]
    pub fn ground_below(&mut self, x: f32, z: f32, from_y: f32) -> Option<f32> {
        if !(x.is_finite() && z.is_finite() && from_y.is_finite()) {
            return None;
        }
        self.query.update(&self.colliders);
        let ray = Ray::new(Point3::new(x, from_y, z), Vector3::new(0.0, -1.0, 0.0));
        self.query
            .cast_ray(
                &self.bodies,
                &self.colliders,
                &ray,
                1000.0,
                true,
                QueryFilter::default(),
            )
            .map(|(_, distance)| from_y - distance)
    }

    /// Puts a walker in the world (replacing any earlier one).
    ///
    /// # Errors
    /// [`Error::BadValue`] for a capsule that is not finite, shorter than its width, or larger than 10 m.
    pub fn set_walker(&mut self, desc: &WalkerDesc) -> Result<(), Error> {
        let numbers = [
            desc.height,
            desc.radius,
            desc.position[0],
            desc.position[1],
            desc.position[2],
        ];
        if !numbers.iter().all(|v| v.is_finite())
            || desc.radius <= 0.0
            || desc.height < desc.radius * 2.0
            || desc.height > 10.0
        {
            return Err(Error::BadValue(
                "a walker needs a finite capsule at least as tall as it is wide, up to 10 m",
            ));
        }
        let half_height = (desc.height * 0.5 - desc.radius).max(0.0);
        let controller = KinematicCharacterController {
            offset: CharacterLength::Absolute(0.01),
            autostep: Some(CharacterAutostep {
                max_height: CharacterLength::Absolute(0.3),
                min_width: CharacterLength::Absolute(0.2),
                include_dynamic_bodies: false,
            }),
            snap_to_ground: Some(CharacterLength::Absolute(0.2)),
            max_slope_climb_angle: 50.0_f32.to_radians(),
            min_slope_slide_angle: 55.0_f32.to_radians(),
            ..KinematicCharacterController::default()
        };
        self.walker = Some(Walker {
            shape: SharedShape::capsule_y(half_height, desc.radius),
            position: Vector3::new(desc.position[0], desc.position[1], desc.position[2]),
            vertical_speed: 0.0,
            grounded: false,
            controller,
        });
        Ok(())
    }

    /// Moves the walker for one step: `velocity` is the wanted horizontal speed in metres per second (X and Z), `jump` the
    /// upward speed to start a jump with (only used while on the ground). Gravity, slopes and steps are handled here.
    ///
    /// # Errors
    /// [`Error::NoSuchBody`] when there is no walker, or [`Error::BadValue`] for a speed that is not finite.
    pub fn walk(&mut self, velocity: [f32; 2], jump: Option<f32>) -> Result<WalkerState, Error> {
        if !velocity.iter().chain(jump.iter()).all(|v| v.is_finite()) {
            return Err(Error::BadValue("walker speeds must be finite"));
        }
        self.query.update(&self.colliders);
        let gravity_y = self.gravity[1];
        let Some(walker) = self.walker.as_mut() else {
            return Err(Error::NoSuchBody);
        };
        if walker.grounded && walker.vertical_speed < 0.0 {
            walker.vertical_speed = 0.0;
        }
        if let (true, Some(speed)) = (walker.grounded, jump) {
            walker.vertical_speed = speed;
        }
        walker.vertical_speed += gravity_y * STEP;
        let desired = Vector3::new(
            velocity[0] * STEP,
            walker.vertical_speed * STEP,
            velocity[1] * STEP,
        );
        let at = Isometry3::translation(walker.position.x, walker.position.y, walker.position.z);
        let moved = walker.controller.move_shape(
            STEP,
            &self.bodies,
            &self.colliders,
            &self.query,
            walker.shape.as_ref(),
            &at,
            desired,
            QueryFilter::default(),
            |_| {},
        );
        walker.position += moved.translation;
        walker.grounded = moved.grounded;
        // Bumping a ceiling stops the rise.
        if walker.vertical_speed > 0.0 && moved.translation.y < desired.y * 0.5 {
            walker.vertical_speed = 0.0;
        }
        Ok(WalkerState {
            position: [walker.position.x, walker.position.y, walker.position.z],
            grounded: walker.grounded,
            vertical_speed: walker.vertical_speed,
        })
    }
}

#[cfg(test)]
mod tests;
