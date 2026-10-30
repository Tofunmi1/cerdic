//! Hands in the game (ADR 0062 phase 4): an input can make an entity follow a tracked controller, kinds can be grabbable, a squeezed grip
//! picks up the nearest grabbable thing in reach and carries it until the grip lets go, and a pulled trigger raises a `use` event on whatever
//! is in reach. Everything else (sockets, levers, doors) is ordinary rules over those events and over `touch`.
//! Written before the bodies; do not weaken.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup unwraps data that is known to be valid"
)]
use ve_game::{GameDef, HandInput, HandSide, Inputs, World};

const GAME: &str = "game name=hands
var grabbed=0
var released=0
var used=0
var seated=0
kind glove_l size=0.08 tags=hand_l part=glove
kind glove_r size=0.08 tags=hand_r part=glove
kind fuse size=0.1 tags=item part=fuse_mesh grab=1
kind rock size=0.1 tags=scenery
kind lever size=0.15 tags=control
kind socket size=0.12 tags=port
spawn glove_l at=0,0,0
spawn glove_r at=0,0,0
spawn fuse at=0,1,0.5
spawn rock at=0,1,-0.5
spawn lever at=2,1,0
spawn socket at=-2,1,0
input lh follow=left_hand target=tag:hand_l
input rh follow=right_hand target=tag:hand_r
rule on=grab do=add:grabbed:1
rule on=release do=add:released:1
rule on=use a=tag:hand_r b=tag:control do=add:used:1
rule on=touch a=kind:fuse b=kind:socket do=add:seated:1;destroy:a
";

fn hand(side: HandSide, position: [f64; 3], grip: bool, trigger: bool) -> HandInput {
    HandInput {
        side,
        position,
        grip,
        trigger,
    }
}

fn with(hands: Vec<HandInput>) -> Inputs {
    Inputs {
        hands,
        ..Inputs::default()
    }
}

fn world() -> World {
    World::new(&GameDef::parse(GAME).unwrap(), 1)
}

fn at(world: &World, kind: &str) -> [f64; 3] {
    world
        .entities()
        .iter()
        .find(|e| e.kind == kind)
        .map(|e| e.position)
        .unwrap()
}

fn var(world: &World, name: &str) -> f64 {
    world.vars()[name]
}

#[test]
fn hands_follow_their_controllers_and_stay_put_when_untracked() {
    let mut w = world();
    w.step(&with(vec![
        hand(HandSide::Left, [-0.3, 1.2, -0.4], false, false),
        hand(HandSide::Right, [0.3, 1.1, -0.4], false, false),
    ]))
    .unwrap();
    assert_eq!(at(&w, "glove_l"), [-0.3, 1.2, -0.4]);
    assert_eq!(at(&w, "glove_r"), [0.3, 1.1, -0.4]);
    w.step(&with(vec![hand(
        HandSide::Right,
        [0.5, 1.0, 0.0],
        false,
        false,
    )]))
    .unwrap();
    assert_eq!(at(&w, "glove_r"), [0.5, 1.0, 0.0]);
    assert_eq!(
        at(&w, "glove_l"),
        [-0.3, 1.2, -0.4],
        "an untracked hand stays where it was"
    );
}

#[test]
fn a_grip_squeezed_in_reach_picks_up_the_grabbable_thing_and_carries_it() {
    let mut w = world();
    // The fuse is at 0,1,0.5; the hand arrives within reach (0.08 + 0.1) with the grip open, then squeezes.
    w.step(&with(vec![hand(
        HandSide::Right,
        [0.0, 1.0, 0.4],
        false,
        false,
    )]))
    .unwrap();
    assert_eq!(var(&w, "grabbed"), 0.0, "an open hand grabs nothing");
    w.step(&with(vec![hand(
        HandSide::Right,
        [0.0, 1.0, 0.4],
        true,
        false,
    )]))
    .unwrap();
    assert_eq!(var(&w, "grabbed"), 1.0, "squeezing in reach grabs");
    // Carried: the fuse keeps its offset from the hand (0.1 further along z) while the grip stays closed.
    w.step(&with(vec![hand(
        HandSide::Right,
        [1.0, 1.5, -1.0],
        true,
        false,
    )]))
    .unwrap();
    let fuse = at(&w, "fuse");
    assert!(
        (fuse[0] - 1.0).abs() < 1e-9
            && (fuse[1] - 1.5).abs() < 1e-9
            && (fuse[2] - -0.9).abs() < 1e-9,
        "carried with its offset: {fuse:?}"
    );
    assert_eq!(var(&w, "released"), 0.0);
}

#[test]
fn letting_go_releases_the_item_where_it_is_and_it_stops_following() {
    let mut w = world();
    w.step(&with(vec![hand(
        HandSide::Right,
        [0.0, 1.0, 0.4],
        true,
        false,
    )]))
    .unwrap();
    w.step(&with(vec![hand(
        HandSide::Right,
        [0.5, 1.2, 0.0],
        true,
        false,
    )]))
    .unwrap();
    w.step(&with(vec![hand(
        HandSide::Right,
        [0.5, 1.2, 0.0],
        false,
        false,
    )]))
    .unwrap();
    assert_eq!(var(&w, "released"), 1.0);
    let dropped = at(&w, "fuse");
    w.step(&with(vec![hand(
        HandSide::Right,
        [3.0, 3.0, 3.0],
        false,
        false,
    )]))
    .unwrap();
    assert_eq!(
        at(&w, "fuse"),
        dropped,
        "a released item no longer follows the hand"
    );
}

#[test]
fn only_grabbable_kinds_within_reach_are_picked_up() {
    let mut w = world();
    // Beside the rock (not grabbable): nothing happens.
    w.step(&with(vec![hand(
        HandSide::Right,
        [0.0, 1.0, -0.5],
        true,
        false,
    )]))
    .unwrap();
    assert_eq!(var(&w, "grabbed"), 0.0, "a rock is not grabbable");
    // Too far from the fuse: nothing.
    let mut w = world();
    w.step(&with(vec![hand(
        HandSide::Right,
        [0.0, 1.0, 1.2],
        true,
        false,
    )]))
    .unwrap();
    assert_eq!(var(&w, "grabbed"), 0.0, "out of reach");
}

#[test]
fn a_grip_that_was_already_closed_on_arrival_does_not_grab() {
    let mut w = world();
    w.step(&with(vec![hand(
        HandSide::Right,
        [3.0, 1.0, 3.0],
        true,
        false,
    )]))
    .unwrap();
    w.step(&with(vec![hand(
        HandSide::Right,
        [0.0, 1.0, 0.45],
        true,
        false,
    )]))
    .unwrap();
    assert_eq!(
        var(&w, "grabbed"),
        0.0,
        "grabbing happens when the grip closes, not when a closed hand passes over"
    );
}

#[test]
fn an_item_in_one_hand_cannot_be_taken_by_the_other() {
    let mut w = world();
    w.step(&with(vec![hand(
        HandSide::Right,
        [0.0, 1.0, 0.4],
        true,
        false,
    )]))
    .unwrap();
    // The left hand closes on the same fuse a moment later, within reach: it gets nothing.
    w.step(&with(vec![
        hand(HandSide::Right, [0.0, 1.0, 0.4], true, false),
        hand(HandSide::Left, [0.0, 1.0, 0.6], true, false),
    ]))
    .unwrap();
    assert_eq!(var(&w, "grabbed"), 1.0, "one grab, not two");
    w.step(&with(vec![
        hand(HandSide::Right, [2.0, 1.0, 2.0], true, false),
        hand(HandSide::Left, [0.0, 1.0, 0.6], true, false),
    ]))
    .unwrap();
    assert!(
        (at(&w, "fuse")[0] - 2.0).abs() < 1e-9,
        "it went with the first hand"
    );
}

#[test]
fn a_carried_item_touches_things_so_a_socket_can_take_it_and_the_hand_is_then_free() {
    let mut w = world();
    w.step(&with(vec![hand(
        HandSide::Right,
        [0.0, 1.0, 0.4],
        true,
        false,
    )]))
    .unwrap();
    w.step(&with(vec![hand(
        HandSide::Right,
        [-1.9, 1.0, -0.1],
        true,
        false,
    )]))
    .unwrap();
    assert_eq!(var(&w, "seated"), 1.0, "carried into the socket");
    assert!(
        w.entities().iter().all(|e| e.kind != "fuse"),
        "the rule consumed it"
    );
    w.step(&with(vec![hand(
        HandSide::Right,
        [-1.9, 1.0, -0.1],
        false,
        false,
    )]))
    .unwrap();
    assert_eq!(
        var(&w, "released"),
        0.0,
        "an item that no longer exists is not released"
    );
}

#[test]
fn pulling_the_trigger_raises_use_on_what_is_in_reach_once_per_pull() {
    let mut w = world();
    w.step(&with(vec![hand(
        HandSide::Right,
        [2.0, 1.0, 0.1],
        false,
        true,
    )]))
    .unwrap();
    assert_eq!(var(&w, "used"), 1.0, "a pull beside the lever uses it");
    w.step(&with(vec![hand(
        HandSide::Right,
        [2.0, 1.0, 0.1],
        false,
        true,
    )]))
    .unwrap();
    assert_eq!(
        var(&w, "used"),
        1.0,
        "holding the trigger is not another pull"
    );
    w.step(&with(vec![hand(
        HandSide::Right,
        [2.0, 1.0, 0.1],
        false,
        false,
    )]))
    .unwrap();
    w.step(&with(vec![hand(
        HandSide::Right,
        [2.0, 1.0, 0.1],
        false,
        true,
    )]))
    .unwrap();
    assert_eq!(var(&w, "used"), 2.0, "a second pull counts");
    w.step(&with(vec![hand(
        HandSide::Right,
        [-5.0, 1.0, 5.0],
        false,
        false,
    )]))
    .unwrap();
    w.step(&with(vec![hand(
        HandSide::Right,
        [-5.0, 1.0, 5.0],
        false,
        true,
    )]))
    .unwrap();
    assert_eq!(var(&w, "used"), 2.0, "a pull in empty air uses nothing");
}

#[test]
fn the_left_hand_does_not_trigger_a_rule_written_for_the_right() {
    let mut w = world();
    w.step(&with(vec![hand(
        HandSide::Left,
        [2.0, 1.0, 0.1],
        false,
        true,
    )]))
    .unwrap();
    assert_eq!(var(&w, "used"), 0.0);
}

#[test]
fn holds_survive_snapshots_and_the_same_inputs_give_the_same_game() {
    let steps = [
        with(vec![hand(HandSide::Right, [0.0, 1.0, 0.4], true, false)]),
        with(vec![hand(HandSide::Right, [0.3, 1.1, 0.1], true, false)]),
        with(vec![hand(HandSide::Right, [0.6, 1.2, -0.2], true, false)]),
    ];
    let mut a = world();
    a.step(&steps[0]).unwrap();
    let saved = a.snapshot();
    a.step(&steps[1]).unwrap();
    a.restore(&saved).unwrap();
    a.step(&steps[1]).unwrap();
    a.step(&steps[2]).unwrap();
    let mut b = world();
    for step in &steps {
        b.step(step).unwrap();
    }
    assert_eq!(
        format!("{:?}", a.snapshot()),
        format!("{:?}", b.snapshot()),
        "restoring in the middle of a carry changes nothing"
    );
    assert!(
        (at(&a, "fuse")[0] - 0.6).abs() < 1e-9,
        "and the carry went on after the restore"
    );
}

#[test]
fn grab_and_hand_inputs_survive_printing_and_unknown_hands_are_refused() {
    let def = GameDef::parse(GAME).unwrap();
    let text = def.to_text();
    assert!(
        text.contains("grab=1")
            && text.contains("follow=left_hand")
            && text.contains("follow=right_hand"),
        "{text}"
    );
    assert_eq!(GameDef::parse(&text).unwrap(), def);
    let errors =
        GameDef::parse(&GAME.replace("follow=left_hand", "follow=third_hand")).unwrap_err();
    assert!(
        errors.iter().any(|e| e.message.contains("follow")),
        "{errors:?}"
    );
}
