//! Transitions, one-shots, interruption and the presets.

use crate::blend::Pose;
use crate::graph::{Condition, MAX_LAYERS, StateMachine, Transition};
use crate::machine::Machine;
use crate::preset::{FALL, IDLE, JUMP, Preset, RUN, WALK};

use super::{clip_state, lengths, total, weight};

fn mix(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x100_0000_01b3);
    }
}

fn hash(poses: &[Pose]) -> u64 {
    let mut out = 0xcbf2_9ce4_8422_2325_u64;
    for pose in poses {
        for layer in &pose.layers {
            mix(&mut out, layer.clip.as_bytes());
            mix(&mut out, &layer.time.to_bits().to_le_bytes());
            mix(&mut out, &layer.weight.to_bits().to_le_bytes());
        }
        mix(&mut out, &[0xff]);
    }
    out
}

#[test]
fn transition_fires_and_crossfades_to_zero() {
    let graph = StateMachine {
        entry: "a".to_owned(),
        extras: crate::Extras::default(),
        states: vec![clip_state("a", IDLE, true), clip_state("b", RUN, true)],
        transitions: vec![Transition {
            from: Some("a".to_owned()),
            to: "b".to_owned(),
            conditions: vec![Condition::Greater {
                parameter: "speed".to_owned(),
                value: 0.5,
            }],
            duration: 0.4,
            exit_time: None,
        }],
    };
    graph.validate().unwrap();
    let mut machine = Machine::new(graph, &lengths());
    machine.set_float("speed", 1.0);

    let start = machine.step(0.1);
    assert!(
        (weight(&start, IDLE) - 1.0).abs() < 1e-4,
        "no pop when it starts"
    );
    assert!(weight(&start, RUN) < 1e-4);

    let mid = machine.step(0.1);
    assert!(weight(&mid, RUN) > 0.0 && weight(&mid, RUN) < 1.0);
    assert!((total(&mid) - 1.0).abs() < 1e-4);

    let mut last = mid;
    for _ in 0..3 {
        last = machine.step(0.1);
    }
    assert!(
        weight(&last, IDLE) < 1e-6,
        "the source reaches zero exactly at the end"
    );
    assert!((weight(&last, RUN) - 1.0).abs() < 1e-4);
}

#[test]
fn trigger_fires_once_and_clears() {
    let graph = StateMachine {
        entry: "a".to_owned(),
        extras: crate::Extras::default(),
        states: vec![
            clip_state("a", IDLE, true),
            clip_state("once", "jump", false),
        ],
        transitions: vec![Transition {
            from: Some("a".to_owned()),
            to: "once".to_owned(),
            conditions: vec![Condition::Trigger {
                parameter: "go".to_owned(),
            }],
            duration: 0.0,
            exit_time: None,
        }],
    };
    graph.validate().unwrap();
    let mut machine = Machine::new(graph, &lengths());
    machine.trigger("go");
    machine.step(0.01);
    assert_eq!(machine.current_state(), Some("once"));
    assert!(!machine.params().triggered("go"), "the trigger is consumed");
    machine.step(0.01);
    assert_eq!(machine.current_state(), Some("once"));
}

#[test]
fn one_shot_clamps_at_its_last_time() {
    let graph = StateMachine {
        entry: "once".to_owned(),
        extras: crate::Extras::default(),
        states: vec![clip_state("once", "jump", false)],
        transitions: Vec::new(),
    };
    graph.validate().unwrap();
    let mut machine = Machine::new(graph, &lengths());
    let mut pose = machine.step(0.0);
    for _ in 0..40 {
        pose = machine.step(0.05);
    }
    assert!(
        (pose.layers[0].time - 0.5).abs() < 1e-4,
        "clamped at the clip length"
    );
    assert!((weight(&pose, "jump") - 1.0).abs() < 1e-4);
}

#[test]
fn exit_time_returns_after_the_one_shot() {
    let graph = StateMachine {
        entry: "a".to_owned(),
        extras: crate::Extras::default(),
        states: vec![
            clip_state("a", IDLE, true),
            clip_state("once", "jump", false),
        ],
        transitions: vec![
            Transition {
                from: Some("a".to_owned()),
                to: "once".to_owned(),
                conditions: vec![Condition::Trigger {
                    parameter: "go".to_owned(),
                }],
                duration: 0.0,
                exit_time: None,
            },
            Transition {
                from: Some("once".to_owned()),
                to: "a".to_owned(),
                conditions: Vec::new(),
                duration: 0.0,
                exit_time: Some(0.9),
            },
        ],
    };
    graph.validate().unwrap();
    let mut machine = Machine::new(graph, &lengths());
    machine.trigger("go");
    machine.step(0.01);
    assert_eq!(machine.current_state(), Some("once"));
    let mut returned = false;
    for _ in 0..40 {
        machine.step(0.05);
        if machine.current_state() == Some("a") {
            returned = true;
            break;
        }
    }
    assert!(
        returned,
        "the exit-time transition fired after the one-shot"
    );
}

#[test]
fn interrupting_a_crossfade_has_no_pop() {
    let graph = StateMachine {
        entry: "a".to_owned(),
        extras: crate::Extras::default(),
        states: vec![
            clip_state("a", IDLE, true),
            clip_state("b", RUN, true),
            clip_state("c", WALK, true),
        ],
        transitions: vec![
            Transition {
                from: Some("a".to_owned()),
                to: "b".to_owned(),
                conditions: vec![Condition::Trigger {
                    parameter: "tob".to_owned(),
                }],
                duration: 1.0,
                exit_time: None,
            },
            Transition {
                from: Some("b".to_owned()),
                to: "c".to_owned(),
                conditions: vec![Condition::Trigger {
                    parameter: "toc".to_owned(),
                }],
                duration: 1.0,
                exit_time: None,
            },
        ],
    };
    graph.validate().unwrap();
    let mut machine = Machine::new(graph, &lengths());
    machine.step(0.0);
    machine.trigger("tob");
    let mut previous = machine.step(0.1);
    assert!((weight(&previous, IDLE) - 1.0).abs() < 1e-4);
    machine.trigger("toc");
    for _ in 0..40 {
        let pose = machine.step(0.1);
        assert!((total(&pose) - 1.0).abs() < 1e-3);
        for clip in [IDLE, RUN, WALK] {
            assert!(
                (weight(&pose, clip) - weight(&previous, clip)).abs() < 0.35,
                "no pop on {clip}"
            );
        }
        previous = pose;
    }
    assert!(
        weight(&previous, WALK) > 0.9,
        "the interrupting target takes over"
    );
}

/// Ten seconds of the locomotion preset under a speed ramp, with one jump trigger.
fn locomotion_run() -> Vec<Pose> {
    let preset = Preset::Locomotion;
    let graph = preset.graph();
    graph.validate().unwrap();
    let mut machine = Machine::new(graph, &preset.clip_lengths());
    machine.set_bool("grounded", true);
    let dt = 1.0 / 60.0;
    let mut elapsed = 0.0_f32;
    let mut poses = Vec::new();
    for step in 0..600 {
        let speed = if elapsed < 5.0 {
            elapsed
        } else {
            10.0 - elapsed
        };
        machine.set_float("speed", speed.max(0.0));
        if step == 120 {
            machine.trigger("jump");
        }
        poses.push(machine.step(dt));
        elapsed += dt;
    }
    poses
}

#[test]
fn scripts_are_deterministic() {
    let first = locomotion_run();
    let second = locomotion_run();
    assert_eq!(hash(&first), hash(&second));
    assert_eq!(first, second);
}

#[test]
fn a_locomotion_jump_plays_the_jump_clip() {
    let preset = Preset::Locomotion;
    let lengths = preset.clip_lengths();
    assert_eq!(lengths.get(JUMP), Some(&0.8), "the jump is 0.8 s");
    assert_eq!(lengths.get(FALL), Some(&0.6), "the fall is 0.6 s");
    let graph = preset.graph();
    graph.validate().unwrap();
    let mut machine = Machine::new(graph, &lengths);
    machine.set_bool("grounded", true);
    machine.set_float("speed", 0.0);
    machine.step(0.1);
    machine.trigger("jump");
    let mut pose = machine.step(1.0 / 60.0);
    for _ in 0..30 {
        pose = machine.step(1.0 / 60.0);
    }
    assert_eq!(machine.current_state(), Some("jump"));
    assert!(weight(&pose, JUMP) > 0.9, "the pose plays the jump clip");
}

#[test]
fn presets_validate_and_run_a_speed_ramp() {
    for preset in [Preset::Locomotion, Preset::Flyer, Preset::Door] {
        let graph = preset.graph();
        graph.validate().unwrap();
        let mut machine = Machine::new(preset.graph(), &preset.clip_lengths());
        machine.set_bool("grounded", true);
        let dt = 1.0 / 60.0;
        let mut elapsed = 0.0_f32;
        for _ in 0..600 {
            let speed = if elapsed < 5.0 {
                elapsed
            } else {
                10.0 - elapsed
            };
            machine.set_float("speed", speed.max(0.0));
            let pose = machine.step(dt);
            assert!((total(&pose) - 1.0).abs() < 1e-3, "{preset:?}");
            assert!(pose.layers.len() <= MAX_LAYERS, "{preset:?}");
            for layer in &pose.layers {
                assert!(
                    layer.weight.is_finite() && layer.time.is_finite(),
                    "{preset:?}"
                );
                assert!(layer.weight >= 0.0 && layer.weight <= 1.0, "{preset:?}");
            }
            elapsed += dt;
        }
    }
}
