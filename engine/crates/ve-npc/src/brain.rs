use std::collections::BTreeMap;

use thiserror::Error;

use crate::{Blackboard, Rng, Value};

pub(crate) mod tree;
pub use tree::BehaviorTree;

/// Result of evaluating one behaviour tree node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Node completed successfully.
    Success,
    /// Node could not complete its goal.
    Failure,
    /// Node needs a later tick to continue.
    Running,
}

/// A command emitted by an NPC action.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Move toward a point.
    MoveTo {
        /// Destination point.
        target: [f32; 3],
    },
    /// Stop movement.
    Stop,
    /// Look toward a point.
    LookAt {
        /// Point to face.
        target: [f32; 3],
    },
    /// Speak text.
    Say {
        /// Spoken text.
        text: String,
    },
    /// Play a named animation clip.
    Play {
        /// Animation clip name.
        clip: String,
    },
    /// Use a named target.
    Use {
        /// Target name.
        target: String,
    },
    /// Emit a named event.
    Emit {
        /// Event name.
        event: String,
    },
    /// Emit a game-specific command.
    Custom {
        /// Game-specific command name.
        name: String,
        /// Command arguments.
        args: BTreeMap<String, Value>,
    },
}

/// Mutable values available during one deterministic brain tick.
pub struct Ctx {
    /// NPC blackboard.
    pub blackboard: Blackboard,
    /// Seeded random generator.
    pub rng: Rng,
    /// Commands emitted during execution.
    pub commands: Vec<Command>,
    /// Current simulation time in seconds.
    pub now: f64,
    /// Time step in seconds.
    pub dt: f64,
}

impl Ctx {
    /// Creates a tick context with zero time and no commands.
    #[must_use]
    pub fn new(blackboard: Blackboard, rng: Rng) -> Self {
        Self {
            blackboard,
            rng,
            commands: Vec::new(),
            now: 0.0,
            dt: 0.0,
        }
    }
}

/// Errors while building, validating or running a behaviour tree.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum BrainError {
    /// Invalid JSON or node fields.
    #[error("bad behaviour tree spec: {0}")]
    BadSpec(String),
    /// Names absent from the condition/action registry.
    #[error("unknown behaviour tree names: {0:?}")]
    UnknownNames(Vec<String>),
    /// Recursive subtree reference.
    #[error("subtree cycle involving {0}")]
    Cycle(String),
    /// The per-tick node visit budget was exceeded.
    #[error("behaviour tree tick exceeded node budget {budget}")]
    TickBudget {
        /// Configured maximum node visits.
        budget: usize,
    },
}
