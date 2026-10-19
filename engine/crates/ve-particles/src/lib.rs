//! Deterministic CPU particle simulation for billboard renderers.
//!
//! The integrator uses a fixed 1/120 second step, a splitmix64 integer random stream and arithmetic-only
//! approximations. It deliberately avoids platform libm trigonometric, exponential and power functions.
mod preset;
mod simulation;
mod spec;

pub use preset::Preset;
pub use simulation::{Particle, ParticleInstance, ParticleSystem, STEP};
pub use spec::{Colour, Curve, CurvePoint, EmitterSpec, Error, Ground, Shape, Vec3};

#[cfg(test)]
mod tests;
