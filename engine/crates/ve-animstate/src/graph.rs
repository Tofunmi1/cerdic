//! The serializable state machine graph.

use serde::{Deserialize, Serialize};

/// Most states one machine may have.
pub const MAX_STATES: usize = 64;
/// Most transitions one machine may have.
pub const MAX_TRANSITIONS: usize = 256;
/// Most points one 1D blend space may have.
pub const MAX_BLEND_POINTS: usize = 16;
/// Most clip layers one pose may carry; a pose with more drops its lightest clips.
pub const MAX_LAYERS: usize = 8;

/// Why [`StateMachine::validate`] refused a graph.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    /// A state is wrong. `state` names it.
    #[error("state {state:?}: {reason}")]
    State {
        /// Name of the state, or the repeated name the graph gave twice.
        state: String,
        /// What to change.
        reason: String,
    },
    /// A transition is wrong. `index` is its position in the transition list.
    #[error("transition {index}: {reason}")]
    Transition {
        /// Position in [`StateMachine::transitions`], counted from zero.
        index: usize,
        /// What to change.
        reason: String,
    },
    /// The machine as a whole is wrong.
    #[error("machine: {reason}")]
    Machine {
        /// What to change.
        reason: String,
    },
    /// A parameter name is used as two different kinds.
    #[error("parameter {parameter:?}: {reason}")]
    Parameter {
        /// The parameter name.
        parameter: String,
        /// What to change.
        reason: String,
    },
}

/// What a state plays.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Motion {
    /// One clip, played at `speed` (`1.0` is authored speed); `looping` wraps its time.
    Clip {
        /// Name of the clip (empty is the character maker's default walk clip).
        clip: String,
        /// Playback speed multiplier.
        speed: f32,
        /// Whether the clip time wraps.
        looping: bool,
    },
    /// A 1D blend space sampled by a float parameter.
    Blend1D {
        /// Name of the float parameter that picks the point.
        parameter: String,
        /// Ordered blend points; each is a parameter value and the clip fully weighted there.
        points: Vec<BlendPoint>,
    },
}

/// A clip at one value of a 1D blend parameter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlendPoint {
    /// Parameter value at which this clip is fully weighted.
    pub value: f32,
    /// Clip played at this value.
    pub clip: String,
}

/// A named state of the machine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// Unique name used by the entry point and transitions.
    pub name: String,
    /// What the state plays.
    pub motion: Motion,
}

/// A test on the current parameters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Condition {
    /// The float parameter is strictly greater than `value`.
    Greater {
        /// Float parameter name.
        parameter: String,
        /// Threshold.
        value: f32,
    },
    /// The float parameter is strictly less than `value`.
    Less {
        /// Float parameter name.
        parameter: String,
        /// Threshold.
        value: f32,
    },
    /// The bool parameter equals `value`.
    Equals {
        /// Bool parameter name.
        parameter: String,
        /// Required value.
        value: bool,
    },
    /// The named trigger is raised; it is lowered when the transition fires.
    Trigger {
        /// Trigger parameter name.
        parameter: String,
    },
    /// Every inner condition holds.
    And(Vec<Condition>),
}

/// One edge of the machine. The first transition in the list whose conditions hold fires.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Transition {
    /// Source state, or `None` for any state.
    pub from: Option<String>,
    /// Target state.
    pub to: String,
    /// Every condition must hold.
    pub conditions: Vec<Condition>,
    /// Crossfade length in seconds; `0` snaps instantly.
    pub duration: f32,
    /// Fires only once the source one-shot clip has played this fraction (`0..=1`); `None` ignores it.
    pub exit_time: Option<f32>,
}

/// A serializable animation state machine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StateMachine {
    /// States, unique by name.
    pub states: Vec<State>,
    /// Edges, evaluated in order.
    pub transitions: Vec<Transition>,
    /// Name of the state the machine starts in.
    pub entry: String,
    /// Overlay layers (a wave over a walk) and clip events (footsteps).
    #[serde(default)]
    pub extras: crate::layers::Extras,
}
