//! Rigid-body physics behind a small interface of our own (ADR 0035).
//!
//! The rest of the engine never sees Rapier's types: it describes bodies ([`BodyDesc`]), steps a [`World`] at the fixed
//! 1/60 s step and reads poses and contacts back. Every list that crosses the boundary is ordered by body id, so the
//! result of a step is the same wherever it runs.
mod joint;
pub mod ragdoll;
mod shape;
pub mod vehicle;
mod walker;
mod world;

pub use joint::{JointDesc, JointKind, MAX_JOINTS};
pub use shape::Shape;
pub use walker::{WalkerDesc, WalkerState};
pub use world::{Pose, World};
pub use world::{RayHit, ShapeHit, SurfaceHit};

/// Seconds per step: physics runs at the same fixed rate as behaviours.
pub const STEP: f32 = 1.0 / 60.0;
/// Most bodies in one world.
pub const MAX_BODIES: usize = 4096;
/// Most points in a convex or mesh shape.
pub const MAX_SHAPE_POINTS: usize = 65_536;

/// How a body moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Never moves; other bodies collide with it.
    Static,
    /// Moves under gravity and collisions.
    Dynamic,
    /// Moved by the caller each step (a hand, an animated platform); pushes dynamic bodies but nothing pushes it.
    Kinematic,
}

/// Collision groups a body belongs to and accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layers {
    /// Bits identifying this body's groups.
    pub member: u32,
    /// Groups this body accepts.
    pub mask: u32,
}

impl Default for Layers {
    fn default() -> Self {
        Self {
            member: 1,
            mask: u32::MAX,
        }
    }
}

/// A body to add to the world.
#[derive(Clone, Debug)]
pub struct BodyDesc {
    /// What moves it.
    pub kind: Kind,
    /// What it collides as.
    pub shape: Shape,
    /// Where the shape's centre starts, in metres.
    pub position: [f32; 3],
    /// Orientation at the start, a unit quaternion `[x, y, z, w]`.
    pub rotation: [f32; 4],
    /// Mass per cubic metre (water is 1000, wood 600; 1 gives light props).
    pub density: f32,
    /// Sliding friction, 0 (ice) to about 2.
    pub friction: f32,
    /// Bounciness, 0 (dead) to 1 (perfect).
    pub restitution: f32,
    /// Fraction of linear speed lost per second (air drag).
    pub linear_damping: f32,
    /// Fraction of spin lost per second.
    pub angular_damping: f32,
    /// Collision group membership and interaction mask.
    pub layers: Layers,
    /// Whether this collider reports intersections without blocking bodies.
    pub sensor: bool,
}

impl Default for BodyDesc {
    fn default() -> Self {
        Self {
            kind: Kind::Dynamic,
            shape: Shape::Sphere { radius: 0.5 },
            position: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            density: 1.0,
            friction: 0.6,
            restitution: 0.0,
            linear_damping: 0.0,
            angular_damping: 0.05,
            layers: Layers::default(),
            sensor: false,
        }
    }
}

/// Which body, in the order bodies were added.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BodyId(pub usize);

/// Why a body or a step was refused.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    /// The world already holds [`MAX_BODIES`] bodies.
    #[error("a physics world holds at most {MAX_BODIES} bodies")]
    TooManyBodies,
    /// The shape could not be built (no volume, too many points, or a mesh on a moving body).
    #[error("bad shape: {0}")]
    BadShape(&'static str),
    /// A number was not finite or out of range.
    #[error("bad value: {0}")]
    BadValue(&'static str),
    /// The id names no body.
    #[error("no such body")]
    NoSuchBody,
}
