//! Deterministic procedural foot placement for bodies with one or more legs.

mod math;

use thiserror::Error;

/// A leg's body-frame hip and nominal foot placement.
#[derive(Clone, Debug, PartialEq)]
pub struct Leg {
    /// Hip offset from the body position, in the body frame.
    pub hip: [f64; 3],
    /// Standing foot offset from the body position; only x and z are used.
    pub rest_foot: [f64; 3],
    /// Maximum distance from hip to foot, in metres.
    pub reach: f64,
    /// Starting offset in the gait cycle, in the range 0..1.
    pub phase: f64,
}

/// Timing and geometry shared by every leg.
#[derive(Clone, Debug, PartialEq)]
pub struct GaitSpec {
    /// Leg layouts.
    pub legs: Vec<Leg>,
    /// Distance travelled per full gait cycle, in metres.
    pub stride_length: f64,
    /// Maximum swing lift above ground, in metres.
    pub step_height: f64,
    /// Fraction of a cycle spent planted, in the range (0, 1].
    pub duty: f64,
}

/// Position and heading of the body in world coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyPose {
    /// World position in metres.
    pub position: [f64; 3],
    /// Yaw in radians about +y.
    pub yaw: f64,
}

/// World-space foot target and its support state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Foot {
    /// World position in metres.
    pub position: [f64; 3],
    /// Whether the foot is in its support portion of the cycle.
    pub planted: bool,
}

/// Invalid gait configuration.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum GaitError {
    /// The configuration has no legs.
    #[error("gait must have at least one leg")]
    NoLegs,
    /// The configuration exceeds the supported leg count.
    #[error("gait has too many legs (maximum {max})")]
    TooManyLegs {
        /// Maximum supported leg count.
        max: usize,
    },
    /// A numeric setting is not finite.
    #[error("{0} must be finite")]
    NotFinite(&'static str),
    /// A numeric setting is outside its supported range.
    #[error("{0} is outside its supported range")]
    OutOfRange(&'static str),
}

#[derive(Clone, Copy)]
struct LegState {
    foot: Foot,
    lift_from: [f64; 3],
    land_at: [f64; 3],
}

/// Stateful gait cycle and current world-space foot targets.
pub struct Gait {
    spec: GaitSpec,
    cycle: f64,
    states: Vec<LegState>,
    foot_cache: Vec<Foot>,
    initialized: bool,
}

impl Gait {
    /// Creates a gait after checking its dimensions and numeric settings.
    ///
    /// # Errors
    ///
    /// Returns [`GaitError`] when the leg count or any configuration value is invalid.
    pub fn new(spec: GaitSpec) -> Result<Self, GaitError> {
        validate(&spec)?;
        let states: Vec<LegState> = spec
            .legs
            .iter()
            .map(|_| LegState {
                foot: Foot {
                    position: [0.0; 3],
                    planted: false,
                },
                lift_from: [0.0; 3],
                land_at: [0.0; 3],
            })
            .collect();
        let foot_cache = states.iter().map(|state| state.foot).collect();
        Ok(Self {
            spec,
            cycle: 0.0,
            states,
            foot_cache,
            initialized: false,
        })
    }

    /// Advances the gait and updates each target against the supplied ground surface.
    pub fn step(
        &mut self,
        body: BodyPose,
        velocity: [f64; 3],
        dt: f64,
        ground: &dyn Fn(f64, f64) -> f64,
    ) {
        let speed = velocity[0].hypot(velocity[2]);
        if speed > 0.0 {
            self.cycle = (self.cycle + speed * dt / self.spec.stride_length).rem_euclid(1.0);
        }
        if !self.initialized {
            for (leg, state) in self.spec.legs.iter().zip(&mut self.states) {
                let nominal = rest_point(body, leg.rest_foot);
                let hip = world_point(body, leg.hip);
                state.foot.position = clamp_reach(
                    [nominal[0], ground(nominal[0], nominal[2]), nominal[2]],
                    hip,
                    leg.reach,
                );
                state.foot.planted = true;
            }
            self.initialized = true;
        }
        for (index, leg) in self.spec.legs.iter().enumerate() {
            let phase = (self.cycle + leg.phase).rem_euclid(1.0);
            let state = &mut self.states[index];
            let hip = world_point(body, leg.hip);
            let nominal = rest_point(body, leg.rest_foot);
            let is_planted = phase < self.spec.duty;
            if speed == 0.0 {
                state.foot.position = clamp_reach(
                    [nominal[0], ground(nominal[0], nominal[2]), nominal[2]],
                    hip,
                    leg.reach,
                );
                state.foot.planted = true;
            } else if self.spec.duty < 1.0 {
                if !state.foot.planted && is_planted {
                    let target = landing(nominal, velocity, speed, &self.spec, ground);
                    state.foot.position = clamp_reach(target, hip, leg.reach);
                } else if state.foot.planted && !is_planted {
                    state.lift_from = state.foot.position;
                    state.land_at = clamp_reach(
                        landing(nominal, velocity, speed, &self.spec, ground),
                        hip,
                        leg.reach,
                    );
                }
                state.foot.planted = is_planted;
                if !is_planted {
                    let progress =
                        ((phase - self.spec.duty) / (1.0 - self.spec.duty)).clamp(0.0, 1.0);
                    let eased = math::smoothstep(progress);
                    let x = math::lerp(state.lift_from[0], state.land_at[0], eased);
                    let z = math::lerp(state.lift_from[2], state.land_at[2], eased);
                    let y = ground(x, z) + self.spec.step_height * math::sine_arc(progress);
                    state.foot.position = clamp_reach([x, y, z], hip, leg.reach);
                }
            } else {
                state.foot.planted = true;
            }
            if state.foot.planted {
                let distance = distance(state.foot.position, hip);
                if distance > leg.reach {
                    state.foot.position = clamp_reach(state.foot.position, hip, leg.reach);
                    state.foot.planted = false;
                }
            }
            self.foot_cache[index] = state.foot;
        }
    }

    /// Returns the current feet in leg order.
    #[must_use]
    pub fn feet(&self) -> &[Foot] {
        &self.foot_cache
    }
}

fn validate(spec: &GaitSpec) -> Result<(), GaitError> {
    if spec.legs.is_empty() {
        return Err(GaitError::NoLegs);
    }
    if spec.legs.len() > 32 {
        return Err(GaitError::TooManyLegs { max: 32 });
    }
    finite("stride_length", spec.stride_length)?;
    finite("step_height", spec.step_height)?;
    finite("duty", spec.duty)?;
    if spec.stride_length <= 0.0 {
        return Err(GaitError::OutOfRange("stride_length"));
    }
    if spec.step_height < 0.0 {
        return Err(GaitError::OutOfRange("step_height"));
    }
    if !(0.0 < spec.duty && spec.duty <= 1.0) {
        return Err(GaitError::OutOfRange("duty"));
    }
    for leg in &spec.legs {
        for (name, value) in [("hip", leg.hip), ("rest_foot", leg.rest_foot)] {
            for component in value {
                finite(name, component)?;
            }
        }
        finite("reach", leg.reach)?;
        finite("phase", leg.phase)?;
        if leg.reach <= 0.0 {
            return Err(GaitError::OutOfRange("reach"));
        }
        if !(0.0..1.0).contains(&leg.phase) {
            return Err(GaitError::OutOfRange("phase"));
        }
    }
    Ok(())
}

fn finite(name: &'static str, value: f64) -> Result<(), GaitError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(GaitError::NotFinite(name))
    }
}

fn world_point(body: BodyPose, local: [f64; 3]) -> [f64; 3] {
    let (sin, cos) = body.yaw.sin_cos();
    [
        body.position[0] + local[0] * cos + local[2] * sin,
        body.position[1] + local[1],
        body.position[2] - local[0] * sin + local[2] * cos,
    ]
}

fn rest_point(body: BodyPose, rest: [f64; 3]) -> [f64; 3] {
    world_point(body, rest)
}

fn landing(
    nominal: [f64; 3],
    velocity: [f64; 3],
    speed: f64,
    spec: &GaitSpec,
    ground: &dyn Fn(f64, f64) -> f64,
) -> [f64; 3] {
    let lead = spec.duty * spec.stride_length / speed * 0.5;
    let x = nominal[0] + velocity[0] * lead;
    let z = nominal[2] + velocity[2] * lead;
    [x, ground(x, z), z]
}

fn clamp_reach(point: [f64; 3], hip: [f64; 3], reach: f64) -> [f64; 3] {
    let delta = [point[0] - hip[0], point[1] - hip[1], point[2] - hip[2]];
    let distance = distance(point, hip);
    if distance <= reach {
        point
    } else {
        let scale = reach / distance;
        [
            hip[0] + delta[0] * scale,
            hip[1] + delta[1] * scale,
            hip[2] + delta[2] * scale,
        ]
    }
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}
