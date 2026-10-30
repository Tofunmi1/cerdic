//! Public inputs and errors for the bounded 3D liquid reference solver.
use super::{MAX_PARTICLES, MAX_STEP};

/// Particle state in world metres and metres per second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    /// Position in world coordinates.
    pub position: [f32; 3],
    /// Velocity in world coordinates.
    pub velocity: [f32; 3],
}

/// Fixed parameters for a closed 3D liquid volume.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Inclusive lower collision bounds.
    pub min: [f32; 3],
    /// Inclusive upper collision bounds.
    pub max: [f32; 3],
    /// SPH kernel radius in metres.
    pub smoothing_radius: f32,
    /// Rest density in kilograms per cubic metre.
    pub rest_density: f32,
    /// Per-particle mass in kilograms.
    pub particle_mass: f32,
    /// Constant acceleration in metres per second squared.
    pub gravity: [f32; 3],
    /// Exponential velocity damping per second.
    pub damping: f32,
    /// Number of density projection iterations per step (1..=8).
    pub iterations: u8,
}

/// Invalid volume construction or time step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// The initial particle set is empty.
    #[error("a 3D liquid needs at least one particle")]
    Empty,
    /// The configured or initial particle count exceeds the CPU reference budget.
    #[error("3D liquid particle count exceeds {MAX_PARTICLES}")]
    ParticleLimit,
    /// A physical parameter is invalid.
    #[error("invalid 3D liquid parameter: {0}")]
    Parameter(&'static str),
    /// An initial particle is outside the closed simulation bounds.
    #[error("initial particle {0} is outside the liquid bounds")]
    Outside(usize),
    /// The step is not finite, positive, or within the supported bound.
    #[error("3D liquid step must be finite and in (0, {MAX_STEP}]")]
    Step,
}
