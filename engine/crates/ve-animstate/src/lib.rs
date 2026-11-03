//! Deterministic animation state machine core.
//!
//! Generated characters have named clips (the walk is `""`, plus `run`, `idle`, `jump`, `fall` and
//! `fly`) but nothing chooses between them. This crate is the pure logic core: it reads named parameters and decides
//! which clips play and with what weights, as a [`Pose`] of [`ClipWeight`] layers. Wiring that to the
//! runtime and the graph language is a separate task.
//!
//! The path is arithmetic only (`+ - * /` and a smoothstep polynomial), so the same inputs and step
//! sequence give bit-identical output on every platform, native or wasm. Every map is a
//! [`std::collections::BTreeMap`] so iteration is ordered.
//!
//! ```
//! use ve_animstate::{Machine, Preset};
//!
//! let preset = Preset::Locomotion;
//! let graph = preset.graph();
//! graph.validate()?;
//! let mut machine = Machine::new(graph, &preset.clip_lengths());
//! machine.set_bool("grounded", true);
//! machine.set_float("speed", 2.7);
//! let pose = machine.step(1.0 / 60.0);
//! let total: f32 = pose.layers.iter().map(|layer| layer.weight).sum();
//! assert!((total - 1.0).abs() < 1e-4);
//! # Ok::<(), ve_animstate::Error>(())
//! ```

mod blend;
mod events;
mod graph;
mod layers;
mod machine;
mod machine_extras;
mod params;
mod preset;
mod text;
mod text_extras;
mod trs;
mod validate;

pub use blend::{ClipWeight, Pose};
pub use events::crossed;
pub use layers::{
    ClipEvent, EVENT_WEIGHT, Extras, FiredEvent, Layer, LayerWeight, MAX_EVENTS, MAX_OVERLAYS,
    OverlayWeight, plain_name,
};
pub use graph::{
    BlendPoint, Condition, Error, MAX_BLEND_POINTS, MAX_LAYERS, MAX_STATES, MAX_TRANSITIONS,
    Motion, State, StateMachine, Transition,
};
pub use machine::{MAX_STEP, Machine};
pub use params::Params;
pub use preset::{FALL, FLY, IDLE, JUMP, Preset, RUN, WALK};
pub use text::ParseError;
pub use trs::{Trs, apply_additive, apply_override, blend_trs};

#[cfg(test)]
mod tests;
