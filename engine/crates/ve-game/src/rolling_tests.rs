//! Supplementary coverage for rolling boundaries, rollback, and hand state.
use crate::{GameDef, GroundField, HandInput, HandRotation, HandSide, Inputs, World};
use std::sync::Arc;

const TOOL: &str = "game name=rolling
var strokes=0
kind tool size=0.05 swing=1 reach=0.9 tags=hand
kind ball size=0.03 roll=1
input rh follow=right_hand target=tag:hand
spawn tool at=0,0,0
spawn ball at=0.25,0.03,-0.9
rule on=swing do=add:strokes:1
";

fn world(text: &str) -> World {
    World::new(&GameDef::parse(text).unwrap(), 1)
}

#[test]
fn obsolete_kind_keys_and_events_are_rejected() {
    for statement in [
        "kind a strike=1",
        "kind a cup=1",
        "rule on=strike do=emit:hit",
        "rule on=sink do=emit:captured",
    ] {
        assert!(GameDef::parse(&format!("game name=rolling\n{statement}\n")).is_err());
    }
}
fn field(height: f32) -> Arc<GroundField> {
    Arc::new(GroundField::new([-10.0, -10.0], 20.0, 2, 2, vec![height; 4], vec![0; 4]).unwrap())
}
fn hand(x: f64) -> Inputs {
    Inputs {
        hands: vec![HandInput {
            side: HandSide::Right,
            position: [x, 0.03, 0.0],
            grip: false,
            trigger: false,
        }],
        ..Inputs::default()
    }
}
fn run(world: &mut World, steps: usize) {
    for _ in 0..steps {
        world.step(&Inputs::default()).unwrap();
    }
}

#[test]
fn field_outer_edges_nearest_samples_and_invalid_coordinates() {
    let ground = GroundField::new(
        [0.0, 0.0],
        1.0,
        2,
        2,
        vec![0.0, 1.0, 2.0, 3.0],
        vec![1, 2, 3, 4],
    )
    .unwrap();
    assert!((ground.height(1.0, 1.0).unwrap() - 3.0).abs() < f64::EPSILON);
    assert_eq!(ground.surface(1.0, 1.0), 4);
    assert_eq!(ground.surface(0.49, 0.51), 3);
    assert_eq!(ground.surface(0.51, 0.49), 2);
    assert!(ground.height(f64::NAN, 0.0).is_none());
    assert!(ground.height(0.0, f64::INFINITY).is_none());
    assert_eq!(
        ground.normal(2.0, 0.0).map(f64::to_bits),
        [0.0_f64, 1.0, 0.0].map(f64::to_bits)
    );
    for (origin, cell) in [
        ([f64::NAN, 0.0], 1.0),
        ([0.0, f64::INFINITY], 1.0),
        ([0.0; 2], f64::INFINITY),
        ([0.0; 2], -1.0),
    ] {
        assert!(GroundField::new(origin, cell, 2, 2, vec![0.0; 4], vec![0; 4]).is_err());
    }
    assert!(GroundField::new([0.0; 2], 1.0, usize::MAX, 2, vec![], vec![]).is_err());
}

#[test]
fn rolling_numeric_boundaries_name_and_locate_bad_keys() {
    for (statement, key) in [
        ("kind a swing=maybe", "swing"),
        ("kind a roll=2", "roll"),
        ("kind a reach=NaN", "reach"),
        ("kind a loft=bad", "loft"),
        ("kind a power=5.01", "power"),
        ("surface sand parts=g decel=inf", "decel"),
        ("surface sand parts=g bounce=-0.01", "bounce"),
        ("terrain parts=g cell=NaN", "cell"),
        ("terrain parts=g,", "parts"),
    ] {
        let diagnostics =
            GameDef::parse(&format!("game name=rolling\n  {statement}\n")).unwrap_err();
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.line, 2);
        assert_eq!(
            diagnostic.column,
            3 + statement.find(&format!("{key}=")).unwrap()
        );
        assert!(diagnostic.message.contains(key));
        assert!(!diagnostic.suggestion.is_empty());
    }
    let definition = GameDef::parse("game name=rolling\nkind ball roll=1 swing=1 pocket=1 reach=0 loft=80 power=5\nsurface sand parts=a,b decel=8 bounce=0\nterrain parts=a,b\n").unwrap();
    assert_eq!(definition.surfaces()[0].parts, ["a", "b"]);
    assert!((definition.surfaces()[0].decel - 8.0).abs() < f64::EPSILON);
    assert!((definition.terrain().unwrap().cell - 0.25).abs() < f64::EPSILON);
    assert_eq!(GameDef::parse(&definition.describe()).unwrap(), definition);
}

#[test]
fn tool_history_and_latches_replay_after_restoring_a_mid_swing_snapshot() {
    let mut w = world(TOOL);
    for x in [-1.0, -0.75, -0.5, -0.25] {
        w.step(&hand(x)).unwrap();
    }
    let before = w.snapshot();
    for x in [0.0, 0.25, 0.5, 0.75] {
        w.step(&hand(x)).unwrap();
    }
    let hash = w.hash();
    let entities = w.entities().to_vec();
    assert!((w.vars()["strokes"] - 1.0).abs() < f64::EPSILON);
    w.restore(&before).unwrap();
    for x in [0.0, 0.25, 0.5, 0.75] {
        w.step(&hand(x)).unwrap();
    }
    assert_eq!(w.hash(), hash);
    assert_eq!(w.entities(), entities);
    let latched = w.snapshot();
    for x in [1.0, 1.25, 1.5] {
        w.step(&hand(x)).unwrap();
    }
    let hash = w.hash();
    assert!((w.vars()["strokes"] - 1.0).abs() < f64::EPSILON);
    w.restore(&latched).unwrap();
    for x in [1.0, 1.25, 1.5] {
        w.step(&hand(x)).unwrap();
    }
    assert_eq!(w.hash(), hash);
}

#[test]
fn host_terrain_is_not_hashed_or_restored_and_out_latches_are_restored() {
    let mut w = world(
        "game name=rolling\nvar outs=0\nkind ball size=0.03 roll=1\nspawn ball at=0,1,0\nrule on=out do=add:outs:1\n",
    );
    let hash = w.hash();
    let before = w.snapshot();
    let ground = field(f32::NAN);
    w.set_ground(Some(Arc::clone(&ground)));
    assert_eq!(w.hash(), hash);
    w.restore(&before).unwrap();
    assert!(Arc::ptr_eq(w.ground().unwrap(), &ground));
    w.step(&Inputs::default()).unwrap();
    assert!((w.vars()["outs"] - 1.0).abs() < f64::EPSILON);
    let out = w.snapshot();
    run(&mut w, 10);
    let hash = w.hash();
    w.restore(&out).unwrap();
    run(&mut w, 10);
    assert_eq!(w.hash(), hash);
    assert!((w.vars()["outs"] - 1.0).abs() < f64::EPSILON);
    w.restore(&before).unwrap();
    run(&mut w, 10);
    assert!((w.vars()["outs"] - 1.0).abs() < f64::EPSILON);
}

#[test]
fn orientation_defaults_on_missing_rotation_and_tracking_loss_does_not_sweep_a_teleport() {
    let mut w = world(TOOL);
    let mut input = hand(-1.0);
    input.hand_rotations.push(HandRotation {
        side: HandSide::Right,
        rotation: [0.0, 0.0, 1.0, 0.0],
    });
    w.step(&input).unwrap();
    assert_eq!(
        w.entities()[0].orientation.map(f64::to_bits),
        [0.0_f64, 0.0, 1.0, 0.0].map(f64::to_bits)
    );
    w.step(&hand(-0.5)).unwrap();
    assert_eq!(
        w.entities()[0].orientation.map(f64::to_bits),
        [0.0_f64, 0.0, 0.0, 1.0].map(f64::to_bits)
    );
    w.step(&input).unwrap();
    w.step(&Inputs::default()).unwrap();
    assert_eq!(
        w.entities()[0].orientation.map(f64::to_bits),
        [0.0_f64, 0.0, 0.0, 1.0].map(f64::to_bits)
    );
    w.step(&hand(1.0)).unwrap();
    assert!(w.vars()["strokes"].abs() < f64::EPSILON);
}

#[test]
fn roll_and_grab_carry_without_terrain_motion() {
    let mut w = world(
        "game name=rolling\nkind glove size=0.1 tags=hand\nkind ball size=0.03 roll=1 grab=1\ninput rh follow=right_hand target=tag:hand\nspawn glove at=0,0,0\nspawn ball at=0,1,0\n",
    );
    w.set_ground(Some(field(0.0)));
    let mut input = hand(0.0);
    input.hands[0].position[1] = 1.0;
    input.hands[0].grip = true;
    w.step(&input).unwrap();
    for i in 0..30 {
        input.hands[0].position = [f64::from(i) * 0.05, 1.0, 0.0];
        w.step(&input).unwrap();
    }
    assert!((w.entities()[1].position[1] - 1.0).abs() < 1e-9);
    assert!((w.entities()[1].position[0] - 1.45).abs() < 1e-9);
}

#[test]
fn a_failing_swing_rule_rolls_back_rolling_state() {
    let mut w = world(&format!(
        "{TOOL}rule on=swing do=emit:loop\nrule on=emit:loop do=emit:loop\n"
    ));
    w.step(&hand(-0.5)).unwrap();
    w.step(&hand(0.0)).unwrap();
    let hash = w.hash();
    let entities = w.entities().to_vec();
    for _ in 0..2 {
        assert!(w.step(&hand(0.5)).is_err());
        assert_eq!(w.hash(), hash);
        assert_eq!(w.entities(), entities);
    }
}

#[test]
fn non_rolling_keeps_the_legacy_digest() {
    let mut w = world("game name=a\nkind coin size=0.4\nspawn coin at=1,0,0\n");
    run(&mut w, 30);
    // Pre-rolling FNV byte stream: tick, RNG, next id, status, entity, and two idle hands.
    assert_eq!(w.hash(), 0xe281_f7a0_c5de_9d69);
    w.set_ground(Some(field(3.0)));
    assert_eq!(w.hash(), 0xe281_f7a0_c5de_9d69);
}

#[test]
fn a_swing_uses_the_last_three_step_speeds() {
    let mut w = world(&TOOL.replace("at=0.25,0.03,-0.9", "at=0,0.03,-0.9"));
    for x in [-0.2, -0.19, -0.18, 0.0] {
        w.step(&hand(x)).unwrap();
    }
    // Three displacements total 0.2 m in 0.05 s, then one rolling step slows the launch.
    assert!((w.entities()[1].velocity[0] - (4.0 - 1.0 / 60.0)).abs() < 1e-9);
    assert!((w.vars()["strokes"] - 1.0).abs() < f64::EPSILON);
}

#[test]
fn a_capped_fast_swing_stays_latched_while_the_ball_keeps_up() {
    let mut w = world(&TOOL.replace("at=0.25,0.03,-0.9", "at=0.5,0.03,-0.9"));
    for x in [-1.0, 0.0, 1.0, 2.0, 3.0] {
        w.step(&hand(x)).unwrap();
    }
    assert!((w.vars()["strokes"] - 1.0).abs() < f64::EPSILON);
}
