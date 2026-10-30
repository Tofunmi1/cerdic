//! `StateMachine::parse` and `to_text`: the animation state machine as a few lines of text a model can write, with errors that name the line.
//! The graph node (a later job) carries this text; the runtime (a later job) drives the machine. Written before the body; do not weaken.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_animstate::{BlendPoint, Condition, Motion, Preset, State, StateMachine, Transition};

const LOCOMOTION: &str = "\
# a character that idles, walks and runs by speed, jumps on a trigger and falls when airborne
entry ground
state ground blend=speed idle@0 walk@1.5 run@4
state jump clip=jump once
state fall clip=fall loop
transition any -> jump when trigger:jump fade=0.1
transition any -> fall when grounded=false fade=0.2
transition jump -> ground exit=0.9 fade=0.15 when grounded=true
transition fall -> ground when grounded=true & speed>0.1 fade=0.25
";

fn clip(name: &str, play: &str, speed: f32, looping: bool) -> State {
    State {
        name: name.to_owned(),
        motion: Motion::Clip {
            clip: play.to_owned(),
            speed,
            looping,
        },
    }
}

#[test]
fn a_machine_is_read_into_the_graph_it_describes() {
    let machine = StateMachine::parse(LOCOMOTION).unwrap();
    assert_eq!(machine.entry, "ground");
    assert_eq!(
        machine.states,
        vec![
            State {
                name: "ground".to_owned(),
                motion: Motion::Blend1D {
                    parameter: "speed".to_owned(),
                    points: vec![
                        BlendPoint {
                            value: 0.0,
                            clip: "idle".to_owned()
                        },
                        BlendPoint {
                            value: 1.5,
                            clip: "walk".to_owned()
                        },
                        BlendPoint {
                            value: 4.0,
                            clip: "run".to_owned()
                        },
                    ],
                },
            },
            clip("jump", "jump", 1.0, false),
            clip("fall", "fall", 1.0, true),
        ]
    );
    assert_eq!(
        machine.transitions,
        vec![
            Transition {
                from: None,
                to: "jump".to_owned(),
                conditions: vec![Condition::Trigger {
                    parameter: "jump".to_owned()
                }],
                duration: 0.1,
                exit_time: None,
            },
            Transition {
                from: None,
                to: "fall".to_owned(),
                conditions: vec![Condition::Equals {
                    parameter: "grounded".to_owned(),
                    value: false
                }],
                duration: 0.2,
                exit_time: None,
            },
            Transition {
                from: Some("jump".to_owned()),
                to: "ground".to_owned(),
                conditions: vec![Condition::Equals {
                    parameter: "grounded".to_owned(),
                    value: true
                }],
                duration: 0.15,
                exit_time: Some(0.9),
            },
            Transition {
                from: Some("fall".to_owned()),
                to: "ground".to_owned(),
                conditions: vec![
                    Condition::Equals {
                        parameter: "grounded".to_owned(),
                        value: true
                    },
                    Condition::Greater {
                        parameter: "speed".to_owned(),
                        value: 0.1
                    },
                ],
                duration: 0.25,
                exit_time: None,
            },
        ]
    );
    machine.validate().unwrap();
}

#[test]
fn defaults_are_a_looping_clip_at_normal_speed_and_a_short_fade() {
    let machine = StateMachine::parse(
        "entry a\nstate a clip=idle\nstate b clip=walk speed=1.5\ntransition a -> b when speed>1\n",
    )
    .unwrap();
    assert_eq!(machine.states[0], clip("a", "idle", 1.0, true));
    assert_eq!(machine.states[1], clip("b", "walk", 1.5, true));
    assert!(
        (machine.transitions[0].duration - 0.15).abs() < 1e-6,
        "default fade {}",
        machine.transitions[0].duration
    );
    assert_eq!(machine.transitions[0].exit_time, None);
    assert_eq!(
        machine.transitions[0].conditions,
        vec![Condition::Greater {
            parameter: "speed".to_owned(),
            value: 1.0
        }]
    );
}

#[test]
fn the_default_clip_has_the_empty_name_and_is_written_as_two_quotes() {
    let machine = StateMachine::parse("entry walk\nstate walk clip=\"\"\n").unwrap();
    assert_eq!(machine.states[0], clip("walk", "", 1.0, true));
    assert!(machine.to_text().contains("clip=\"\""));
}

#[test]
fn less_than_and_both_bool_values_and_chained_conditions_read() {
    let text = "entry a\nstate a clip=x\nstate b clip=y\ntransition a -> b when speed<0.5 & grounded=true & trigger:go\n";
    let machine = StateMachine::parse(text).unwrap();
    assert_eq!(
        machine.transitions[0].conditions,
        vec![
            Condition::Less {
                parameter: "speed".to_owned(),
                value: 0.5
            },
            Condition::Equals {
                parameter: "grounded".to_owned(),
                value: true
            },
            Condition::Trigger {
                parameter: "go".to_owned()
            },
        ]
    );
}

#[test]
fn comments_blank_lines_and_extra_spaces_are_ignored() {
    let text = "\n   # note\nentry   a   \n\nstate a    clip=idle   # trailing note\n\n  state b clip=walk\ntransition   a  ->  b   when  speed>1  \n";
    let machine = StateMachine::parse(text).unwrap();
    assert_eq!(machine.states.len(), 2);
    assert_eq!(machine.transitions.len(), 1);
}

#[test]
fn text_round_trips_for_every_preset() {
    for preset in [Preset::Locomotion, Preset::Flyer, Preset::Door] {
        let graph = preset.graph();
        let text = graph.to_text();
        let back = StateMachine::parse(&text).unwrap_or_else(|e| panic!("{preset:?}: {e}\n{text}"));
        assert_eq!(
            back, graph,
            "{preset:?} did not survive its own text:\n{text}"
        );
        back.validate().unwrap();
    }
}

#[test]
fn the_written_text_is_stable() {
    let graph = StateMachine::parse(LOCOMOTION).unwrap();
    let once = graph.to_text();
    let twice = StateMachine::parse(&once).unwrap().to_text();
    assert_eq!(once, twice);
}

fn line_of(text: &str) -> usize {
    StateMachine::parse(text).unwrap_err().line
}

#[test]
fn errors_name_the_line_that_is_wrong() {
    assert_eq!(
        line_of("entry a\nstate a clip=x\nbogus line here\n"),
        3,
        "unknown keyword"
    );
    assert_eq!(
        line_of("entry a\nstate a clip=x\ntransition a -> nowhere when speed>1\n"),
        3,
        "unknown target state"
    );
    assert_eq!(
        line_of("entry a\nstate a clip=x\ntransition ghost -> a when speed>1\n"),
        3,
        "unknown source state"
    );
    assert_eq!(
        line_of("entry a\nstate a clip=x\nstate a clip=y\n"),
        3,
        "a repeated state name"
    );
    assert_eq!(
        line_of("entry a\nstate a clip=x speed=fast\n"),
        2,
        "a bad number"
    );
    assert_eq!(
        line_of("entry a\nstate a clip=x\nstate b clip=y\ntransition a -> b when speed>>1\n"),
        4,
        "a bad condition"
    );
    assert_eq!(
        line_of("entry a\nstate a blend=speed\n"),
        2,
        "a blend with no points"
    );
    assert_eq!(
        line_of("entry a\nstate a blend=speed idle@0 walk\n"),
        2,
        "a blend point without a value"
    );
    assert_eq!(
        line_of("entry a\nstate a\n"),
        2,
        "a state that plays nothing"
    );
    assert_eq!(
        line_of("entry a\nstate a clip=x\nstate b clip=y\ntransition a -> b fade=-1\n"),
        4,
        "a negative fade"
    );
    assert_eq!(
        line_of("entry a\nstate a clip=x\nstate b clip=y\ntransition a -> b exit=1.5\n"),
        4,
        "an exit time over one"
    );
}

#[test]
fn whole_machine_problems_are_errors_too() {
    assert!(StateMachine::parse("").is_err(), "no states");
    assert!(StateMachine::parse("state a clip=x\n").is_err(), "no entry");
    assert!(
        StateMachine::parse("entry missing\nstate a clip=x\n").is_err(),
        "an entry that is not a state"
    );
    let many: String = std::iter::once("entry s0\n".to_owned())
        .chain((0..65).map(|i| format!("state s{i} clip=c\n")))
        .collect();
    assert!(StateMachine::parse(&many).is_err(), "more than 64 states");
    let error = StateMachine::parse("entry a\nstate a clip=x\ntransition a -> zzz when speed>1\n")
        .unwrap_err();
    assert!(
        error.to_string().contains("zzz"),
        "the message names the unknown state: {error}"
    );
    assert!(
        error.to_string().starts_with("line 3"),
        "the message starts with the line: {error}"
    );
}

#[test]
fn nonsense_never_panics() {
    for text in [
        "\u{0}\u{1}\u{2}",
        "entry",
        "state",
        "transition ->",
        "entry a\nstate a clip=\"",
        "state a clip=x speed=1e999\nentry a",
        &"x".repeat(100_000),
        "->->->",
        "entry a b c\n",
    ] {
        let _ = StateMachine::parse(text);
    }
}
