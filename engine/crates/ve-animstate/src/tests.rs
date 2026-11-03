//! Behaviour tests for the state machine core.

mod errors;
mod pose;
mod runtime;

use std::collections::HashMap;

use crate::blend::Pose;
use crate::graph::{BlendPoint, Motion, State, StateMachine};
use crate::preset::{FLY, IDLE, RUN, WALK};

/// Clip lengths used by the tests; `jump` is a one-shot used by the one-shot tests.
fn lengths() -> HashMap<String, f32> {
    HashMap::from([
        (WALK.to_owned(), 1.0),
        (RUN.to_owned(), 0.6),
        (IDLE.to_owned(), 3.0),
        (FLY.to_owned(), 0.8),
        ("jump".to_owned(), 0.5),
    ])
}

fn clip_state(name: &str, clip: &str, looping: bool) -> State {
    State {
        name: name.to_owned(),
        motion: Motion::Clip {
            clip: clip.to_owned(),
            speed: 1.0,
            looping,
        },
    }
}

/// A single blend state: idle at 0, walk at 1.4, run at 4, extrapolated by clamping.
fn blend_graph() -> StateMachine {
    StateMachine {
        entry: "loc".to_owned(),
        extras: crate::Extras::default(),
        states: vec![State {
            name: "loc".to_owned(),
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
        }],
        transitions: Vec::new(),
    }
}

fn weight(pose: &Pose, clip: &str) -> f32 {
    pose.layers
        .iter()
        .find(|layer| layer.clip == clip)
        .map_or(0.0, |layer| layer.weight)
}

fn time_of(pose: &Pose, clip: &str) -> f32 {
    pose.layers
        .iter()
        .find(|layer| layer.clip == clip)
        .map_or(0.0, |layer| layer.time)
}

fn total(pose: &Pose) -> f32 {
    pose.layers.iter().map(|layer| layer.weight).sum()
}
