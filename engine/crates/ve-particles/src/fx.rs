//! Effects: emitters with spawn timing, forces, collisions with a world, sub-emitters and trails, simulated deterministically on the CPU.
//!
//! Plan W4, `docs/plans/engine-libraries-adoption.md`. The effect model (a set of emitters, each with spawn rules, forces, collisions,
//! sub-emitters and a trail) follows Effekseer's node tree (MIT); the simulation is our own. This is the
//! reference path: exact, deterministic, used for tests and for particles that matter to gameplay. Large visual counts will run the same
//! spec on the GPU.
pub mod draw;
mod emit;
mod forces;
mod sim;
mod validate;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use forces::turbulence_field;
pub use sim::{Particle, System};

/// Most emitters in one effect.
pub const MAX_EMITTERS: usize = 16;
/// Most particles one emitter may hold.
pub const MAX_PARTICLES: usize = 100_000;
/// Most points in a trail.
pub const MAX_TRAIL: usize = 64;
/// Most children one sub-emitter trigger may spawn at once.
pub const MAX_CHILDREN: u32 = 1_000;

/// A one-shot birth of `count` particles at `time` seconds into the emitter's own clock.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Burst {
    /// Seconds after the emitter starts (after its delay).
    pub time: f32,
    /// Particles born.
    pub count: u32,
}

/// When and how fast an emitter gives birth.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Spawn {
    /// Births per second while active.
    pub rate: f32,
    /// One-shot births.
    pub bursts: Vec<Burst>,
    /// Seconds before the emitter starts.
    pub delay: f32,
    /// Seconds the emitter stays active; `0` is forever.
    pub duration: f32,
    /// Start over every `duration` seconds (needs a duration).
    pub looping: bool,
}

/// Where particles are born, around the emitter's origin.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Shape {
    /// At the origin.
    #[default]
    Point,
    /// Anywhere in a ball.
    Sphere {
        /// Radius.
        radius: f32,
    },
    /// Anywhere in a box.
    Box {
        /// Half the size along each axis.
        half_extents: [f32; 3],
    },
    /// Anywhere on a flat disc in the XZ plane.
    Disc {
        /// Radius.
        radius: f32,
    },
    /// Along a vertical line.
    Line {
        /// Length, centred on the origin.
        length: f32,
    },
}

/// Something that pushes particles.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Force {
    /// A constant acceleration, metres per second squared.
    Gravity([f32; 3]),
    /// Drags the particle's velocity towards the wind's: `strength` per second (0 none, large locks on at once).
    Wind {
        /// The wind's velocity.
        velocity: [f32; 3],
        /// How hard it drags, per second.
        strength: f32,
    },
    /// Slows the particle: the share of its speed lost per second grows with this number.
    Drag(f32),
    /// Pushes round an axis (right-handed), weaker towards the edge of its `radius`.
    Vortex {
        /// A point on the axis.
        centre: [f32; 3],
        /// The axis direction.
        axis: [f32; 3],
        /// Acceleration at the axis, metres per second squared.
        strength: f32,
        /// Reach.
        radius: f32,
    },
    /// Pulls towards a point, weaker towards the edge of its `radius`.
    Attractor {
        /// Where it pulls to.
        centre: [f32; 3],
        /// Acceleration at the centre, metres per second squared.
        strength: f32,
        /// Reach.
        radius: f32,
    },
    /// A swirling acceleration with no sources or sinks (curl noise).
    Turbulence {
        /// Acceleration scale, metres per second squared.
        strength: f32,
        /// Features per metre.
        scale: f32,
        /// How fast the pattern changes, in noise units per second.
        speed: f32,
    },
}

/// What a particle does when it hits the world.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Reaction {
    /// Rebounds with `restitution` of its speed; `friction` slows the slide.
    Bounce {
        /// 0 (dead stop) to 1 (perfect bounce).
        restitution: f32,
        /// 0 (ice) and up; the slide loses at most this fraction of each impact's normal impulse.
        friction: f32,
    },
    /// Stops where it hits and stays until it dies.
    Stick,
    /// Disappears.
    Kill,
}

/// How an emitter's particles meet the [`World`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Collide {
    /// The particle's size for collisions, metres.
    pub radius: f32,
    /// What happens on a hit.
    pub reaction: Reaction,
}

/// Keep a particle's recent path, for ribbons and streaks.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trail {
    /// Points kept, `2..=64` (the newest is always the particle itself).
    pub points: usize,
    /// A new point is recorded after the particle moves this far, metres.
    pub min_step: f32,
}

/// When a sub-emitter fires.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubTrigger {
    /// A particle is born.
    Birth,
    /// A particle dies of old age.
    Death,
    /// A particle hits the world (an impact, not resting contact).
    Collide,
}

/// Spawns `count` particles of another emitter of the effect where the trigger happens.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubEmit {
    /// When.
    pub on: SubTrigger,
    /// Index of the child emitter in the effect.
    pub emitter: usize,
    /// Particles per trigger, up to 1000.
    pub count: u32,
}

/// How a particle is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Render {
    /// A camera-facing soft round sprite.
    #[default]
    Sprite,
    /// A ribbon along the particle's trail (needs a `trail`).
    Ribbon,
    /// A small lit box that tumbles: debris, shards, leaves of stone or wood.
    Cube,
}

/// How a particle's colour combines with what is behind it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Blend {
    /// Ordinary transparency: smoke, dust.
    #[default]
    Alpha,
    /// Adds light: fire, sparks, magic.
    Additive,
}

/// What a particle looks like over its life: start and end values, blended by age.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Look {
    /// Sprite or ribbon.
    pub render: Render,
    /// How it blends.
    pub blend: Blend,
    /// Size at birth and at death, metres (a ribbon's width).
    pub size: [f32; 2],
    /// Linear colour at birth and at death.
    pub colour: [[f32; 3]; 2],
    /// Opacity at birth and at death, `0..=1`.
    pub alpha: [f32; 2],
    /// Brightness multiplier for glowing effects, `0..=16`.
    pub glow: f32,
    /// How fast a cube tumbles, radians per second, `0..=50`.
    pub spin: f32,
}

impl Default for Look {
    fn default() -> Self {
        Self {
            render: Render::Sprite,
            blend: Blend::Alpha,
            size: [0.2, 0.2],
            colour: [[1.0; 3]; 2],
            alpha: [1.0, 0.0],
            glow: 0.0,
            spin: 3.0,
        }
    }
}

/// One emitter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Emitter {
    /// Spawn timing.
    pub spawn: Spawn,
    /// Lifetime range, seconds.
    pub lifetime: [f32; 2],
    /// Where the shape is centred (added to the position a sub-emitter's trigger happens at).
    pub origin: [f32; 3],
    /// The birth shape.
    pub shape: Shape,
    /// Birth speed range, metres per second.
    pub speed: [f32; 2],
    /// Birth direction.
    pub direction: [f32; 3],
    /// Spread around the direction, radians (approximate; up to about 1.5).
    pub cone: f32,
    /// Forces, applied in order every step.
    pub forces: Vec<Force>,
    /// Collision with the world.
    pub collide: Option<Collide>,
    /// Trail of recent positions.
    pub trail: Option<Trail>,
    /// Sub-emitters.
    pub sub: Vec<SubEmit>,
    /// Particle cap, `1..=100000`.
    pub max_particles: usize,
    /// How particles are drawn.
    pub look: Look,
}

impl Default for Emitter {
    fn default() -> Self {
        Self {
            spawn: Spawn::default(),
            lifetime: [1.0, 1.0],
            origin: [0.0; 3],
            shape: Shape::Point,
            speed: [1.0, 1.0],
            direction: [0.0, 1.0, 0.0],
            cone: 0.0,
            forces: Vec::new(),
            collide: None,
            trail: None,
            sub: Vec::new(),
            max_particles: 1000,
            look: Look::default(),
        }
    }
}

/// A set of emitters that run together; sub-emitters refer to others by index.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Effect {
    /// The emitters.
    pub emitters: Vec<Emitter>,
}

/// A refused effect: the field to change and what is allowed.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum FxError {
    /// A setting is outside its range.
    #[error("invalid {field}: {why}")]
    Invalid {
        /// The setting.
        field: &'static str,
        /// What is allowed.
        why: &'static str,
    },
}

/// Where a particle touched the world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    /// The surface's unit normal, pointing out of the surface.
    pub normal: [f32; 3],
    /// How far the particle is inside the surface, along the normal.
    pub depth: f32,
}

/// The solid things particles collide with: a ground, a heightfield, any shape you can ask "am I inside?".
pub trait World {
    /// The contact of a sphere of `radius` at `position`, if it overlaps something.
    fn contact(&self, position: [f32; 3], radius: f32) -> Option<Contact>;
}

/// Nothing to hit.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoWorld;

impl World for NoWorld {
    fn contact(&self, _: [f32; 3], _: f32) -> Option<Contact> {
        None
    }
}

/// A flat ground at a height.
#[derive(Clone, Copy, Debug)]
pub struct Plane {
    /// World height.
    pub height: f32,
}

impl World for Plane {
    fn contact(&self, position: [f32; 3], radius: f32) -> Option<Contact> {
        let depth = self.height + radius - position[1];
        (depth > 0.0).then_some(Contact {
            normal: [0.0, 1.0, 0.0],
            depth,
        })
    }
}

