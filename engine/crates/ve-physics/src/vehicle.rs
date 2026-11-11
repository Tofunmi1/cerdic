//! Raycast-wheel vehicles: a chassis body held up by spring-damper suspension at each wheel, driven and steered through tyre friction.
//!
//! The model (a ray per wheel, suspension force from compression and its rate, a friction limit
//! of grip times load, engine force on driven wheels, braking) follows the shape of Jolt's wheeled vehicle (MIT);
//! the code is ours on Rapier. Tyre forces act at the height of the chassis centre, so driving hard does not flip the car (an arcade handling choice).
//! Call [`Vehicle::update`] once before each [`World::step`].
use crate::{BodyId, Error, STEP, World};

/// Most wheels on one vehicle.
pub const MAX_WHEELS: usize = 8;
/// Rolling resistance as a share of each wheel's load.
const ROLLING: f32 = 0.03;
/// With the throttle released the engine slows the driven wheels by this share of the engine force.
const ENGINE_BRAKING: f32 = 0.1;

/// One wheel, attached to the chassis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wheel {
    /// Where the suspension is fixed, in the chassis's own frame (+X right, +Y up, +Z forward), metres.
    pub offset: [f32; 3],
    /// Wheel radius, metres.
    pub radius: f32,
    /// How far the wheel can drop below its mount, metres.
    pub suspension: f32,
    /// Spring stiffness, newtons per metre of compression (about 30,000 for a family car).
    pub stiffness: f32,
    /// Damping, newtons per metre per second (about a seventh of the stiffness).
    pub damping: f32,
    /// Tyre grip: the most sideways or driving force is this times the load on the wheel (1 is a road tyre, 0.3 ice).
    pub grip: f32,
    /// Whether the steering turns this wheel.
    pub steers: bool,
    /// Whether the engine drives this wheel.
    pub drives: bool,
}

/// A vehicle to build around an existing chassis body.
#[derive(Clone, Debug)]
pub struct VehicleDesc {
    /// The chassis body (a dynamic body with its own collider).
    pub chassis: BodyId,
    /// The wheels, at most [`MAX_WHEELS`].
    pub wheels: Vec<Wheel>,
    /// The most force the engine pushes with at full throttle, newtons, shared by the driven wheels.
    pub engine_force: f32,
    /// The most force the brakes oppose motion with at full brake, newtons, shared by all wheels.
    pub brake_force: f32,
    /// How far the steered wheels turn at full lock, radians (up to 1).
    pub max_steer: f32,
}

/// What one wheel did on the last update.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WheelState {
    /// Whether the wheel touches the ground.
    pub in_contact: bool,
    /// How far the spring is compressed, metres.
    pub compression: f32,
    /// Where the wheel touches the ground (zero when it does not).
    pub point: [f32; 3],
    /// How far the wheel is turned by the steering, radians.
    pub steer: f32,
}

/// A running vehicle: the controls and the state of its wheels.
#[derive(Clone, Debug)]
pub struct Vehicle {
    desc: VehicleDesc,
    throttle: f32,
    steer: f32,
    brake: f32,
    states: Vec<WheelState>,
}

type V = [f32; 3];

fn add(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(a: V, s: f32) -> V {
    [a[0] * s, a[1] * s, a[2] * s]
}

fn dot(a: V, b: V) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V, b: V) -> V {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn unit(a: V) -> V {
    let length = dot(a, a).sqrt();
    if length > 1.0e-9 { scale(a, 1.0 / length) } else { [0.0, 1.0, 0.0] }
}

fn rotate(q: [f32; 4], v: V) -> V {
    let axis = [q[0], q[1], q[2]];
    let inner = add(cross(axis, v), scale(v, q[3]));
    add(v, scale(cross(axis, inner), 2.0))
}

fn validate(desc: &VehicleDesc) -> Result<(), Error> {
    if desc.wheels.is_empty() || desc.wheels.len() > MAX_WHEELS {
        return Err(Error::BadValue("a vehicle needs 1 to 8 wheels"));
    }
    let ok = |v: f32| v.is_finite();
    for wheel in &desc.wheels {
        let numbers = [wheel.radius, wheel.suspension, wheel.stiffness, wheel.damping, wheel.grip];
        if !(numbers.iter().all(|v| ok(*v)) && wheel.offset.iter().all(|v| ok(*v))) {
            return Err(Error::BadValue("wheel numbers must be finite"));
        }
        if wheel.radius <= 0.0 || wheel.suspension <= 0.0 || wheel.stiffness < 0.0 || wheel.damping < 0.0 || wheel.grip < 0.0 {
            return Err(Error::BadValue("a wheel needs a radius and suspension above 0 and stiffness, damping and grip not negative"));
        }
    }
    if !(ok(desc.engine_force) && ok(desc.brake_force) && ok(desc.max_steer)) || desc.engine_force < 0.0 || desc.brake_force < 0.0 {
        return Err(Error::BadValue("engine and brake force must be finite and not negative"));
    }
    if !(0.0..=1.0).contains(&desc.max_steer) {
        return Err(Error::BadValue("the steering lock must be between 0 and 1 radian"));
    }
    Ok(())
}

impl Vehicle {
    /// Builds the vehicle around its chassis.
    ///
    /// # Errors
    /// [`Error::NoSuchBody`] if the chassis does not exist, [`Error::BadValue`] for numbers that cannot work.
    pub fn new(world: &mut World, desc: &VehicleDesc) -> Result<Self, Error> {
        world.pose(desc.chassis)?;
        validate(desc)?;
        Ok(Self {
            desc: desc.clone(),
            throttle: 0.0,
            steer: 0.0,
            brake: 0.0,
            states: vec![WheelState::default(); desc.wheels.len()],
        })
    }

    /// Sets the controls: `throttle` -1 (reverse) to 1, `steer` -1 (left) to 1 (right), `brake` 0 to 1. Values outside the range are clamped.
    pub fn drive(&mut self, throttle: f32, steer: f32, brake: f32) {
        let clean = |v: f32, lo: f32, hi: f32| if v.is_finite() { v.clamp(lo, hi) } else { 0.0 };
        self.throttle = clean(throttle, -1.0, 1.0);
        self.steer = clean(steer, -1.0, 1.0);
        self.brake = clean(brake, 0.0, 1.0);
    }

    /// What wheel `index` did on the last update (all zero for an index that does not exist).
    #[must_use]
    pub fn wheel(&self, index: usize) -> WheelState {
        self.states.get(index).copied().unwrap_or_default()
    }

    /// Applies the suspension, drive, brake and tyre forces of this step to the chassis. Call before [`World::step`].
    pub fn update(&mut self, world: &mut World) {
        let chassis = self.desc.chassis;
        let (Ok(pose), Ok(mass)) = (world.pose(chassis), world.body_mass(chassis)) else {
            return;
        };
        let (right, up, forward) = (
            rotate(pose.rotation, [1.0, 0.0, 0.0]),
            rotate(pose.rotation, [0.0, 1.0, 0.0]),
            rotate(pose.rotation, [0.0, 0.0, 1.0]),
        );
        #[expect(clippy::cast_precision_loss, reason = "at most 8 wheels")]
        let share = mass / self.desc.wheels.len() as f32;
        let driven = self.desc.wheels.iter().filter(|w| w.drives).count().max(1);
        #[expect(clippy::cast_precision_loss, reason = "at most 8 wheels")]
        let (driven, count) = (driven as f32, self.desc.wheels.len() as f32);
        // Every wheel reads the chassis' velocity from the same starting state and the impulses are applied together afterwards: applied one
        // by one, a corner's impulse turns the chassis and changes what the next corner reads, the wheels correct each other's corrections,
        // and a small leftover drift is never removed.
        let mut pending: Vec<([f32; 3], [f32; 3])> = Vec::with_capacity(self.desc.wheels.len() * 2);
        for (index, wheel) in self.desc.wheels.iter().enumerate() {
            let mount = add(pose.position, rotate(pose.rotation, wheel.offset));
            let reach = wheel.suspension + wheel.radius;
            let Some(hit) = world.raycast_excluding(mount, scale(up, -1.0), reach, u32::MAX, chassis) else {
                self.states[index] = WheelState::default();
                continue;
            };
            let compression = (reach - hit.distance).max(0.0);
            let Ok(velocity) = world.point_velocity(chassis, hit.point) else {
                continue;
            };
            // Spring and damper along the suspension axis; the damper pushes only while compressing, so a wheel never pulls the car down.
            let closing = -dot(velocity, up);
            let load = (wheel.stiffness * compression + wheel.damping * closing).max(0.0);
            pending.push((scale(up, load * STEP), hit.point));
            // Tyre frame on the ground: forward (steered), and sideways.
            let steer = if wheel.steers { self.steer * self.desc.max_steer } else { 0.0 };
            let heading = add(scale(forward, steer.cos()), scale(right, steer.sin()));
            let normal = unit(hit.normal);
            let along = unit(add(heading, scale(normal, -dot(heading, normal))));
            let side = cross(normal, along);
            let (speed_along, speed_side) = (dot(velocity, along), dot(velocity, side));
            let limit = wheel.grip * load * STEP;
            // Sideways: cancel the slide, as far as the tyre can hold.
            let lateral = (-speed_side * share).clamp(-limit, limit);
            // Lengthwise: engine, brakes, rolling resistance and engine braking, as impulses.
            let mut drive = 0.0;
            if wheel.drives {
                drive += if self.throttle == 0.0 {
                    // Engine braking slows the car but never reverses it: it stops at the speed it removes (a fixed push at zero speed
                    // would shove a parked car backwards every step).
                    -speed_along.signum() * (self.desc.engine_force * ENGINE_BRAKING / driven).min(speed_along.abs() * share / STEP)
                } else {
                    self.throttle * self.desc.engine_force / driven
                };
            }
            let resist = |max_force: f32| (-speed_along * share).clamp(-max_force * STEP, max_force * STEP);
            let longitudinal = (drive * STEP + resist(self.brake * self.desc.brake_force / count) + resist(ROLLING * load)).clamp(-limit, limit);
            // Applied at the height of the chassis centre so that cornering and braking do not tip it over.
            let lift = dot([pose.position[0] - hit.point[0], pose.position[1] - hit.point[1], pose.position[2] - hit.point[2]], up);
            let at = add(hit.point, scale(up, lift));
            pending.push((add(scale(along, longitudinal), scale(side, lateral)), at));
            self.states[index] = WheelState {
                in_contact: true,
                compression,
                point: hit.point,
                steer,
            };
        }
        for (impulse, at) in pending {
            let applied = world.apply_impulse_at(chassis, impulse, at);
            debug_assert!(applied.is_ok());
        }
    }
}
