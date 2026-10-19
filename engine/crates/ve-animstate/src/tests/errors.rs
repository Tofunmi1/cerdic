//! Refused graphs name what to change.

use crate::graph::{
    BlendPoint, Condition, MAX_BLEND_POINTS, MAX_STATES, Motion, State, StateMachine, Transition,
};
use crate::preset::{IDLE, RUN, WALK};

use super::{blend_graph, clip_state};

#[test]
fn unknown_target_and_missing_entry_are_named() {
    let mut graph = blend_graph();
    graph.transitions.push(Transition {
        from: Some("loc".to_owned()),
        to: "nope".to_owned(),
        conditions: Vec::new(),
        duration: 0.1,
        exit_time: None,
    });
    let error = graph.validate().unwrap_err().to_string();
    assert!(error.contains("nope"), "{error}");

    let mut graph = blend_graph();
    graph.entry = "nope".to_owned();
    let error = graph.validate().unwrap_err().to_string();
    assert!(error.contains("nope"), "{error}");
}

#[test]
fn bad_states_are_named() {
    let mut graph = blend_graph();
    graph.states[0].motion = Motion::Blend1D {
        parameter: "speed".to_owned(),
        points: Vec::new(),
    };
    assert!(graph.validate().unwrap_err().to_string().contains("loc"));

    let mut graph = blend_graph();
    let duplicate = graph.states[0].clone();
    graph.states.push(duplicate);
    assert!(graph.validate().unwrap_err().to_string().contains("loc"));

    let mut graph = blend_graph();
    graph.states[0].motion = Motion::Blend1D {
        parameter: "speed".to_owned(),
        points: vec![
            BlendPoint {
                value: 1.4,
                clip: WALK.to_owned(),
            },
            BlendPoint {
                value: 0.0,
                clip: IDLE.to_owned(),
            },
        ],
    };
    assert!(graph.validate().unwrap_err().to_string().contains("loc"));
}

#[test]
fn zero_duration_cycle_is_refused() {
    let graph = StateMachine {
        entry: "a".to_owned(),
        states: vec![clip_state("a", IDLE, true), clip_state("b", RUN, true)],
        transitions: vec![
            Transition {
                from: Some("a".to_owned()),
                to: "b".to_owned(),
                conditions: Vec::new(),
                duration: 0.0,
                exit_time: None,
            },
            Transition {
                from: Some("b".to_owned()),
                to: "a".to_owned(),
                conditions: Vec::new(),
                duration: 0.0,
                exit_time: None,
            },
        ],
    };
    let error = graph.validate().unwrap_err().to_string();
    assert!(error.contains("a, b"), "{error}");
}

#[test]
fn conflicting_parameter_kinds_are_named() {
    let graph = StateMachine {
        entry: "loc".to_owned(),
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
                        value: 1.0,
                        clip: RUN.to_owned(),
                    },
                ],
            },
        }],
        transitions: vec![Transition {
            from: None,
            to: "loc".to_owned(),
            conditions: vec![Condition::Equals {
                parameter: "speed".to_owned(),
                value: true,
            }],
            duration: 0.1,
            exit_time: None,
        }],
    };
    assert!(graph.validate().unwrap_err().to_string().contains("speed"));
}

#[test]
fn bounds_are_refused() {
    let mut graph = StateMachine {
        entry: "s0".to_owned(),
        states: Vec::new(),
        transitions: Vec::new(),
    };
    for index in 0..=MAX_STATES {
        graph
            .states
            .push(clip_state(&format!("s{index}"), IDLE, true));
    }
    assert!(graph.validate().unwrap_err().to_string().contains("states"));

    let mut graph = blend_graph();
    let mut points = Vec::new();
    let mut value = 0.0_f32;
    for _ in 0..=MAX_BLEND_POINTS {
        points.push(BlendPoint {
            value,
            clip: IDLE.to_owned(),
        });
        value += 1.0;
    }
    graph.states[0].motion = Motion::Blend1D {
        parameter: "speed".to_owned(),
        points,
    };
    assert!(graph.validate().unwrap_err().to_string().contains("loc"));
}
