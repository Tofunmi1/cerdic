//! Overlay layers and clip events: a wave over a walk, footsteps at the right moments (plan W2, `docs/plans/engine-libraries-adoption.md`).
//!
//! The idea and the two ways to combine (replace the masked parts' motion, or add the clip's change from its first frame) follow ozz-animation's
//! `BlendingJob` layers with per-joint weights and its additive animation builder (MIT); the code is ours.
use serde::{Deserialize, Serialize};

use crate::graph::Error;

/// Most overlay layers one machine may have.
pub const MAX_OVERLAYS: usize = 4;
/// Most events one machine may have.
pub const MAX_EVENTS: usize = 64;
/// A state's clip makes events only while its weight is above this, so a clip that is nearly faded out stays quiet.
pub const EVENT_WEIGHT: f32 = 0.25;

/// What drives an overlay layer's weight.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LayerWeight {
    /// Always this weight, `0` to `1`.
    Fixed(f32),
    /// The named float parameter, clamped to `0..=1` (a missing parameter is `0`).
    Parameter(String),
}

/// A clip played over the machine's own blend, on some of the parts only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    /// Unique name.
    pub name: String,
    /// The clip it plays.
    pub clip: String,
    /// How strongly it applies.
    pub weight: LayerWeight,
    /// Adds the clip's change from its first frame to what the machine plays, instead of replacing it.
    pub additive: bool,
    /// Output names of the parts it acts on, each with everything that hangs from it; empty acts on every part the machine drives.
    pub mask: Vec<String>,
    /// Playback speed multiplier, above `0`.
    pub speed: f32,
    /// Whether the clip wraps; a one-shot holds its last frame.
    pub looping: bool,
    /// Seconds the weight takes to travel from `0` to `1` (or back); `0` snaps.
    pub fade: f32,
}

/// A named moment in a clip (a footstep, a hit frame, a spawn point).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClipEvent {
    /// The clip it belongs to.
    pub clip: String,
    /// What it is called.
    pub name: String,
    /// Seconds into the clip.
    pub time: f32,
}

/// The layers and events of a machine.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Extras {
    /// Overlay layers, applied in order.
    pub layers: Vec<Layer>,
    /// Clip events.
    pub events: Vec<ClipEvent>,
}

/// An overlay layer as it plays this step.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OverlayWeight {
    /// The layer's name.
    pub layer: String,
    /// The clip it plays.
    pub clip: String,
    /// Seconds into the clip.
    pub time: f32,
    /// Its weight now (layers with no weight are left out).
    pub weight: f32,
    /// Whether it adds to the base instead of replacing it.
    pub additive: bool,
    /// The parts it acts on (empty: all).
    pub mask: Vec<String>,
}

/// An event that happened this step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FiredEvent {
    /// The event's name.
    pub name: String,
    /// The clip it came from.
    pub clip: String,
}

fn machine(reason: String) -> Error {
    Error::Machine { reason }
}

/// Whether `name` is a plain identifier: letters, digits, `_`, `-` and `.`.
#[must_use]
pub fn plain_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

impl Extras {
    /// Checks counts, names and numbers.
    ///
    /// # Errors
    /// [`Error::Machine`] naming what to change.
    pub fn validate(&self) -> Result<(), Error> {
        if self.layers.len() > MAX_OVERLAYS {
            return Err(machine(format!("{} layers is more than the {MAX_OVERLAYS} allowed", self.layers.len())));
        }
        if self.events.len() > MAX_EVENTS {
            return Err(machine(format!("{} events is more than the {MAX_EVENTS} allowed", self.events.len())));
        }
        for (index, layer) in self.layers.iter().enumerate() {
            if !plain_name(&layer.name) || self.layers[..index].iter().any(|other| other.name == layer.name) {
                return Err(machine(format!("layer name {:?} must be unique letters, digits, _, - or .", layer.name)));
            }
            let fixed_ok = match &layer.weight {
                LayerWeight::Fixed(w) => w.is_finite() && (0.0..=1.0).contains(w),
                LayerWeight::Parameter(name) => plain_name(name),
            };
            if !(fixed_ok && layer.speed.is_finite() && layer.speed > 0.0 && layer.fade.is_finite() && layer.fade >= 0.0) {
                return Err(machine(format!(
                    "layer {:?}: weight must be 0 to 1 or a parameter name, speed above 0, fade not negative",
                    layer.name
                )));
            }
            if layer.mask.iter().any(String::is_empty) {
                return Err(machine(format!("layer {:?} names an empty part in its mask", layer.name)));
            }
        }
        for event in &self.events {
            if !plain_name(&event.name) || !event.time.is_finite() || event.time < 0.0 {
                return Err(machine(format!("event {:?} needs a plain name and a time of 0 or more", event.name)));
            }
        }
        Ok(())
    }
}
