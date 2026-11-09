//! Effects are data returned to the host; this crate never performs them.
/// Supported host-side effect families, including animation triggers and parameter updates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EffectKind {
    /// Sound effect.
    Sfx,
    /// Particle preset.
    Particles,
    /// Camera shake.
    CameraShake,
    /// User-facing message.
    Message,
    /// Animation state machine trigger.
    Anim,
    /// Set a numeric animation state machine parameter.
    AnimSet,
    /// Set a boolean animation state machine parameter.
    AnimFlag,
    /// Switch to the named camera; `value` is the blend time in seconds (0 cuts).
    Camera,
}
/// A deferred effect request returned to the host for application.
#[derive(Clone, Debug, PartialEq)]
pub struct Effect {
    /// Effect family.
    pub kind: EffectKind,
    /// Host-defined effect name.
    pub name: String,
    /// Optional target entity.
    pub target: Option<u32>,
    /// Optional numeric parameter value; present only for animation parameter updates.
    pub value: Option<f64>,
}
