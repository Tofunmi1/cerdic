//! Ready-made machines built from the clips the character maker emits.

use std::collections::HashMap;

use crate::graph::{BlendPoint, Condition, Motion, State, StateMachine, Transition};

/// The default clip: the character maker writes the walk with an empty name.
pub const WALK: &str = "";
/// The run clip.
pub const RUN: &str = "run";
/// The idle clip.
pub const IDLE: &str = "idle";
/// The flight clip, for winged creatures.
pub const FLY: &str = "fly";
/// The one-shot jump clip.
pub const JUMP: &str = "jump";
/// The looping falling clip.
pub const FALL: &str = "fall";

/// Ready-made machines for common characters and props.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    /// Idle, walk and run blended on `speed`, a `jump` trigger into the one-shot jump clip, and a
    /// `grounded` fall playing the looping fall clip.
    Locomotion,
    /// Idle, flap and glide laddered on `speed`.
    Flyer,
    /// Closed, opening, open and closing driven by the `open` and `close` triggers.
    Door,
}

impl Preset {
    /// The clips this preset plays, each with a sensible length in seconds.
    #[must_use]
    pub fn clip_lengths(self) -> HashMap<String, f32> {
        HashMap::from([
            (WALK.to_owned(), 1.0),
            (RUN.to_owned(), 0.6),
            (IDLE.to_owned(), 3.0),
            (FLY.to_owned(), 0.8),
            (JUMP.to_owned(), 0.8),
            (FALL.to_owned(), 0.6),
        ])
    }

    /// Builds the machine graph; validate it before use.
    #[must_use]
    pub fn graph(self) -> StateMachine {
        match self {
            Self::Locomotion => locomotion(),
            Self::Flyer => flyer(),
            Self::Door => door(),
        }
    }
}

/// A clip state.
fn clip(name: &str, clip: &str, speed: f32, looping: bool) -> State {
    State {
        name: name.to_owned(),
        motion: Motion::Clip {
            clip: clip.to_owned(),
            speed,
            looping,
        },
    }
}

/// A transition with no exit time.
fn edge(from: Option<&str>, to: &str, conditions: Vec<Condition>, duration: f32) -> Transition {
    Transition {
        from: from.map(str::to_owned),
        to: to.to_owned(),
        conditions,
        duration,
        exit_time: None,
    }
}

/// Idle, walk and run blended on `speed`, a one-shot `jump` and a looping `fall`.
fn locomotion() -> StateMachine {
    StateMachine {
        entry: "locomotion".to_owned(),
        extras: crate::Extras::default(),
        states: vec![
            State {
                name: "locomotion".to_owned(),
                motion: Motion::Blend1D {
                    parameter: "speed".to_owned(),
                    points: vec![
                        BlendPoint {
                            value: 0.0,
                            clip: IDLE.to_owned(),
                        },
                        BlendPoint {
                            value: 1.4,
                            clip: WALK.to_owned(),
                        },
                        BlendPoint {
                            value: 4.0,
                            clip: RUN.to_owned(),
                        },
                    ],
                },
            },
            clip("jump", JUMP, 1.0, false),
            clip("fall", FALL, 1.0, true),
        ],
        transitions: vec![
            edge(
                None,
                "fall",
                vec![Condition::Equals {
                    parameter: "grounded".to_owned(),
                    value: false,
                }],
                0.1,
            ),
            edge(
                Some("fall"),
                "locomotion",
                vec![Condition::Equals {
                    parameter: "grounded".to_owned(),
                    value: true,
                }],
                0.15,
            ),
            edge(
                Some("locomotion"),
                "jump",
                vec![Condition::Trigger {
                    parameter: "jump".to_owned(),
                }],
                0.1,
            ),
            Transition {
                from: Some("jump".to_owned()),
                to: "locomotion".to_owned(),
                conditions: Vec::new(),
                duration: 0.15,
                exit_time: Some(1.0),
            },
        ],
    }
}

/// Idle, flap and glide laddered on `speed`.
fn flyer() -> StateMachine {
    let faster = |value: f32| {
        vec![Condition::Greater {
            parameter: "speed".to_owned(),
            value,
        }]
    };
    let slower = |value: f32| {
        vec![Condition::Less {
            parameter: "speed".to_owned(),
            value,
        }]
    };
    StateMachine {
        entry: "idle".to_owned(),
        extras: crate::Extras::default(),
        states: vec![
            clip("idle", IDLE, 1.0, true),
            clip("flap", FLY, 1.0, true),
            clip("glide", FLY, 0.5, true),
        ],
        transitions: vec![
            edge(Some("idle"), "flap", faster(0.5), 0.25),
            edge(Some("flap"), "glide", faster(4.0), 0.25),
            edge(Some("glide"), "flap", slower(4.0), 0.25),
            edge(Some("flap"), "idle", slower(0.5), 0.25),
        ],
    }
}

/// Closed, opening, open and closing driven by triggers.
fn door() -> StateMachine {
    let trigger = |name: &str| {
        vec![Condition::Trigger {
            parameter: name.to_owned(),
        }]
    };
    let finish = |from: &str, to: &str| Transition {
        from: Some(from.to_owned()),
        to: to.to_owned(),
        conditions: Vec::new(),
        duration: 0.1,
        exit_time: Some(0.9),
    };
    StateMachine {
        entry: "closed".to_owned(),
        extras: crate::Extras::default(),
        states: vec![
            clip("closed", IDLE, 1.0, true),
            clip("opening", WALK, 1.0, false),
            clip("open", IDLE, 1.0, true),
            clip("closing", WALK, 1.0, false),
        ],
        transitions: vec![
            edge(Some("closed"), "opening", trigger("open"), 0.1),
            finish("opening", "open"),
            edge(Some("open"), "closing", trigger("close"), 0.1),
            finish("closing", "closed"),
        ],
    }
}
