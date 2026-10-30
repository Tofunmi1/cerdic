//! Things that fall and can be thrown (ADR 0067): a kind with `fall=1` is pulled down by gravity (9.8 m/s^2) and rests on the ground at
//! `ground y=...` (default 0) with its centre one radius (`size`) above it; a grabbed one is carried without falling, and when the grip lets
//! go it leaves the hand with the velocity the hand gave it over its last few steps, so it can be thrown at things (a `touch` rule fires when
//! a flying thing meets another). `bounce=` (0 to 0.95, default 0.3) is how much of its speed it keeps off the ground. Everything is
//! deterministic at the fixed 1/60 s step. Written before the body; do not weaken.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_game::{GameDef, HandInput, HandSide, Inputs, World};

const GAME: &str = "game name=throw
var hits=0
kind glove size=0.08 tags=hand_r part=glove
kind ball size=0.1 tags=item grab=1 fall=1
kind springy size=0.1 tags=item fall=1 bounce=0.6
kind crate size=0.3 tags=scenery
kind target size=0.3 tags=target
spawn glove at=0,0,0
input rh follow=right_hand target=tag:hand_r
rule on=touch a=tag:item b=tag:target do=add:hits:1
";

fn world_with(extra: &str) -> World {
    World::new(&GameDef::parse(&format!("{GAME}{extra}")).unwrap(), 1)
}

fn hand(position: [f64; 3], grip: bool) -> Inputs {
    Inputs {
        hands: vec![HandInput {
            side: HandSide::Right,
            position,
            grip,
            trigger: false,
        }],
        ..Inputs::default()
    }
}

fn pos(world: &World, kind: &str) -> [f64; 3] {
    world
        .entities()
        .iter()
        .find(|e| e.kind == kind)
        .map(|e| e.position)
        .unwrap()
}

fn vel(world: &World, kind: &str) -> [f64; 3] {
    world
        .entities()
        .iter()
        .find(|e| e.kind == kind)
        .map(|e| e.velocity)
        .unwrap()
}

fn run(world: &mut World, steps: usize) {
    for _ in 0..steps {
        world.step(&Inputs::default()).unwrap();
    }
}

#[test]
fn a_falling_kind_drops_and_comes_to_rest_on_the_ground() {
    let mut w = world_with("spawn ball at=0,2,0\n");
    run(&mut w, 120);
    let p = pos(&w, "ball");
    assert!(
        (p[1] - 0.1).abs() < 1e-9,
        "rests one radius above y=0: {p:?}"
    );
    assert_eq!(vel(&w, "ball"), [0.0, 0.0, 0.0], "and has stopped");
}

#[test]
fn it_falls_at_gravity() {
    let mut w = world_with("spawn ball at=0,5,0\n");
    run(&mut w, 30); // half a second: 0.5 * 9.8 * 0.25 = 1.225 m
    let fallen = 5.0 - pos(&w, "ball")[1];
    assert!(
        (fallen - 1.225).abs() < 0.12,
        "fell {fallen} m in half a second"
    );
}

#[test]
fn a_kind_without_fall_stays_where_it_is() {
    let mut w = world_with("spawn crate at=0,2,0\n");
    run(&mut w, 120);
    assert_eq!(pos(&w, "crate"), [0.0, 2.0, 0.0]);
}

#[test]
fn the_ground_height_is_set_by_a_ground_line() {
    let mut w = world_with("ground y=0.5\nspawn ball at=0,2,0\n");
    run(&mut w, 120);
    assert!(
        (pos(&w, "ball")[1] - 0.6).abs() < 1e-9,
        "{:?}",
        pos(&w, "ball")
    );
}

#[test]
fn a_bad_ground_or_fall_or_bounce_is_a_diagnostic_naming_the_key() {
    for (text, key) in [
        ("ground y=low\n", "ground"),
        ("kind a size=0.1 fall=maybe\n", "fall"),
        ("kind a size=0.1 fall=1 bounce=2\n", "bounce"),
    ] {
        let diagnostics = GameDef::parse(&format!("game name=a\n{text}")).unwrap_err();
        assert!(
            diagnostics.iter().any(|d| d.message.contains(key)),
            "{text}: {diagnostics:?}"
        );
    }
}

#[test]
fn a_carried_thing_does_not_fall_while_the_grip_holds_it() {
    let mut w = world_with("spawn ball at=0,1.5,0\n");
    w.step(&hand([0.0, 1.5, 0.0], true)).unwrap();
    for _ in 0..60 {
        w.step(&hand([0.0, 1.5, 0.0], true)).unwrap();
    }
    assert!(
        (pos(&w, "ball")[1] - 1.5).abs() < 1e-9,
        "{:?}",
        pos(&w, "ball")
    );
}

#[test]
fn let_go_of_it_and_it_falls() {
    let mut w = world_with("spawn ball at=0,1.5,0\n");
    for _ in 0..10 {
        w.step(&hand([0.0, 1.5, 0.0], true)).unwrap();
    }
    for _ in 0..90 {
        w.step(&hand([0.0, 1.5, 0.0], false)).unwrap();
    }
    assert!(
        (pos(&w, "ball")[1] - 0.1).abs() < 1e-9,
        "{:?}",
        pos(&w, "ball")
    );
}

/// Swings the hand along +X at `speed` m/s for ten steps, then lets go.
fn throw_along_x(world: &mut World, speed: f64, height: f64) {
    let dt = 1.0 / 60.0;
    world.step(&hand([0.0, height, 0.0], true)).unwrap(); // grips the ball it is in
    for i in 1..=10 {
        world
            .step(&hand([speed * dt * i as f64, height, 0.0], true))
            .unwrap();
    }
    // The grip lets go while the hand is still moving (the item has followed it up to the last step it was gripped).
    world
        .step(&hand([speed * dt * 11.0, height, 0.0], false))
        .unwrap();
}

#[test]
fn it_leaves_the_hand_with_the_hands_velocity() {
    let mut w = world_with("spawn ball at=0,1.3,0\n");
    throw_along_x(&mut w, 3.0, 1.3);
    let v = vel(&w, "ball");
    assert!((v[0] - 3.0).abs() < 0.4, "thrown at 3 m/s along +X: {v:?}");
    assert!(v[1].abs() < 0.5 && v[2].abs() < 0.2, "{v:?}");
}

#[test]
fn a_thrown_thing_flies_an_arc_and_lands_where_physics_says() {
    let mut w = world_with("spawn ball at=0,1.3,0\n");
    throw_along_x(&mut w, 3.0, 1.3);
    let start_x = pos(&w, "ball")[0];
    let mut peak_x = start_x;
    for _ in 0..40 {
        w.step(&Inputs::default()).unwrap();
        peak_x = peak_x.max(pos(&w, "ball")[0]);
    }
    // 1.2 m of fall takes about 0.5 s, at 3 m/s that is 1.5 m, plus a little roll after the bounce.
    let travelled = peak_x - start_x;
    assert!((1.2..2.2).contains(&travelled), "travelled {travelled} m");
}

#[test]
fn bounce_decides_how_much_of_the_fall_comes_back() {
    let mut w = world_with("spawn ball at=-2,3,0\nspawn springy at=2,3,0\n");
    let mut low = f64::MAX;
    let (mut ball_after, mut spring_after) = (0.0_f64, 0.0_f64);
    let mut bounced = [false, false];
    for _ in 0..150 {
        w.step(&Inputs::default()).unwrap();
        for (i, (kind, after)) in [("ball", &mut ball_after), ("springy", &mut spring_after)]
            .into_iter()
            .enumerate()
        {
            let y = pos(&w, kind)[1];
            if y <= 0.11 {
                bounced[i] = true;
            } else if bounced[i] {
                *after = after.max(y);
            }
        }
        low = low.min(pos(&w, "ball")[1]);
    }
    assert!(low >= 0.1 - 1e-9, "never below the ground: {low}");
    assert!(
        spring_after > ball_after + 0.3,
        "bounce=0.6 comes back higher ({spring_after}) than the default 0.3 ({ball_after})"
    );
    assert!(
        spring_after < 2.0,
        "and never as high as it fell from: {spring_after}"
    );
}

#[test]
fn a_thrown_thing_that_meets_a_target_raises_touch() {
    // At 4 m/s the ball is at x = 1.6 m after 0.4 s, by when it has fallen 0.78 m: the target sits on that arc.
    let mut w = world_with("spawn ball at=0,1.3,0\nspawn target at=1.6,0.55,0\n");
    throw_along_x(&mut w, 4.0, 1.3);
    for _ in 0..60 {
        w.step(&Inputs::default()).unwrap();
    }
    assert!(w.vars()["hits"] >= 1.0, "{:?}", w.vars());
}

#[test]
fn the_same_throw_gives_the_same_hash() {
    let make = || {
        let mut w = world_with("spawn ball at=0,1.3,0\nspawn springy at=1,2,0\n");
        throw_along_x(&mut w, 3.0, 1.3);
        run(&mut w, 90);
        w.hash()
    };
    assert_eq!(make(), make());
}
