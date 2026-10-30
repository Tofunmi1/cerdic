//! Rolling things and hand-held tools (ADR 0068; golf, billiards, bowling, hockey and tennis are all games of it): a ball rolls on the ground of a
//! level (a `GroundField` the host builds from the level's terrain), slows by the surface it is on, runs down slopes, drops into pockets; a club,
//! cue, bat or racquet is a hand-held thing whose swept head swings into the ball with the head's velocity (a fast head
//! must not pass through a small ball between two steps). Written before the bodies; do not weaken.
//!
//! Contract (numbers are metres, seconds, radians unless a name says degrees):
//! - `GroundField::new(origin, cell, nx, nz, heights, surfaces)`: `nx` x `nz` height samples, sample `(i, j)` at `origin + (i, j) * cell`
//!   (x, z), stored `j * nx + i`; `NaN` is a void (water, off the course); `surfaces` has one id per sample (0 = the default surface).
//!   `height(x, z)` interpolates bilinearly and is `None` outside the grid or over a void; `normal(x, z)` is the unit surface normal;
//!   `surface(x, z)` is the id of the nearest sample.
//! - Text: `kind NAME ... roll=1 | swing=1 reach=M loft=DEG power=F | pocket=1`, `surface NAME parts=a,b decel=M/S2 bounce=F` (ids are 1, 2, ...
//!   in order; the default surface 0 has decel 1.0 and bounce 1.0), `terrain parts=a,b cell=M` (which parts make the ground field).
//! - A `roll` thing not held: airborne under gravity 9.8 until its lowest point (`size` below its centre) reaches the ground, bounces with
//!   restitution `kind bounce * surface bounce` (a kind's default bounce is 0.3) and tangential speed kept, rolls when the bounce is under
//!   0.6 m/s: the ground follows it (y = ground + size), the surface slows it by `decel` (constant, m/s^2) and the slope accelerates it down the
//!   hill by 5/7 * 9.8 * sin(slope); it rests (velocity zero) when it is slower than one step of decel and the slope's pull is not greater
//!   than the decel. Over a void it raises `out` (a = the ball) once and keeps falling. Without a field the flat `ground y=` plane is the ground.
//! - A `swing` thing follows a hand (as other hand-followed things do); its head is `reach` metres out of the hand along the hand's -Z axis
//!   (the hand's rotation comes from `Inputs::hand_rotations`, identity when absent). Each step the head's segment from last step to now is
//!   tested against every `roll` thing (ball size + club size): a hit when its speed (mean over the last 3 steps) is at least 0.4 m/s sets the
//!   ball's velocity to `speed * power` (at most 45 m/s) along the head's horizontal direction tilted up by `loft` degrees, and raises
//!   `swing` (a = the tool, b = the ball) once per ball until the head has been farther than 0.3 m or slower than 0.3 m/s since.
//! - `pocket=1` things: a `roll` thing within the pocket's `size` horizontally and moving slower than 1.6 m/s, on the ground, raises `pocket`
//!   (a = the pocket, b = the ball) each step it is so.
//! - A hand-followed thing's `Entity::orientation` is the hand's rotation (x, y, z, w), identity otherwise.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use std::f64::consts::FRAC_1_SQRT_2;
use std::sync::Arc;

use ve_game::{GameDef, GroundField, HandInput, HandRotation, HandSide, Inputs, World};

/// A square field `size` x `size` metres centred on the origin, 0.25 m samples, height `h(x, z)` (NaN for a void), surface `s(x, z)`.
fn field(size: f64, h: impl Fn(f64, f64) -> f32, s: impl Fn(f64, f64) -> u8) -> Arc<GroundField> {
    let cell = 0.25;
    let n = (size / cell) as usize + 1;
    let (mut heights, mut surfaces) = (Vec::new(), Vec::new());
    for j in 0..n {
        for i in 0..n {
            let (x, z) = (-size / 2.0 + i as f64 * cell, -size / 2.0 + j as f64 * cell);
            heights.push(h(x, z));
            surfaces.push(s(x, z));
        }
    }
    Arc::new(GroundField::new([-size / 2.0, -size / 2.0], cell, n, n, heights, surfaces).unwrap())
}

fn flat() -> Arc<GroundField> {
    field(20.0, |_, _| 0.0, |_, _| 0)
}

const BALL: &str = "game name=rolling\nkind ball size=0.03 roll=1\n";

fn world(extra: &str, ground: Option<Arc<GroundField>>) -> World {
    let mut w = World::new(&GameDef::parse(&format!("{BALL}{extra}")).unwrap(), 1);
    w.set_ground(ground);
    w
}

fn ball(w: &World) -> ([f64; 3], [f64; 3]) {
    let e = w.entities().iter().find(|e| e.kind == "ball").unwrap();
    (e.position, e.velocity)
}

fn run(w: &mut World, steps: usize) {
    for _ in 0..steps {
        w.step(&Inputs::default()).unwrap();
    }
}

fn speed(v: [f64; 3]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

// ---- the ground field ----

#[test]
fn a_field_interpolates_heights_and_knows_its_edges_and_voids() {
    let f = field(10.0, |x, _| (0.1 * x) as f32, |x, _| u8::from(x > 2.0));
    assert!((f.height(1.0, 0.0).unwrap() - 0.1).abs() < 1e-6);
    assert!(
        (f.height(1.125, 3.3).unwrap() - 0.1125).abs() < 1e-6,
        "between samples: {:?}",
        f.height(1.125, 3.3)
    );
    assert_eq!(f.height(6.0, 0.0), None, "outside the grid");
    let n = f.normal(0.0, 0.0);
    assert!(
        (n[0] + 0.0995).abs() < 1e-3 && (n[1] - 0.995).abs() < 1e-3 && n[2].abs() < 1e-9,
        "{n:?}"
    );
    assert_eq!((f.surface(0.0, 0.0), f.surface(3.0, 0.0)), (0, 1));
    let hole = field(
        10.0,
        |x, z| if x * x + z * z < 1.0 { f32::NAN } else { 0.0 },
        |_, _| 0,
    );
    assert_eq!(hole.height(0.0, 0.0), None, "a void");
    assert_eq!(hole.height(3.0, 0.0), Some(0.0));
}

#[test]
fn a_badly_shaped_field_is_refused() {
    assert!(
        GroundField::new([0.0, 0.0], 0.25, 4, 4, vec![0.0; 15], vec![0; 16]).is_err(),
        "heights do not fill the grid"
    );
    assert!(
        GroundField::new([0.0, 0.0], 0.25, 4, 4, vec![0.0; 16], vec![0; 12]).is_err(),
        "surfaces do not fill the grid"
    );
    assert!(
        GroundField::new([0.0, 0.0], 0.0, 4, 4, vec![0.0; 16], vec![0; 16]).is_err(),
        "a cell of no size"
    );
    assert!(
        GroundField::new([0.0, 0.0], 0.25, 1, 4, vec![0.0; 4], vec![0; 4]).is_err(),
        "a single column"
    );
}

// ---- a ball on the ground ----

#[test]
fn a_dropped_ball_lands_bounces_by_its_restitution_and_comes_to_rest() {
    let mut w = world("spawn ball at=0,1,0\n", Some(flat()));
    let mut peak_after_bounce = 0.0_f64;
    let mut bounced = false;
    for _ in 0..360 {
        w.step(&Inputs::default()).unwrap();
        let y = ball(&w).0[1];
        if y <= 0.031 {
            bounced = true;
        } else if bounced {
            peak_after_bounce = peak_after_bounce.max(y - 0.03);
        }
    }
    // Fell 0.97 m: 4.36 m/s on impact, 30 % of it back is 1.31 m/s and 0.087 m of height.
    assert!(
        (0.05..0.13).contains(&peak_after_bounce),
        "bounced {peak_after_bounce} m"
    );
    let (p, v) = ball(&w);
    assert!(
        (p[1] - 0.03).abs() < 1e-9 && speed(v) == 0.0,
        "at rest on the ground: {p:?} {v:?}"
    );
}

#[test]
fn a_rolling_ball_stops_where_the_surface_decel_says() {
    // 3 m/s on the default surface (decel 1.0): 9 / 2 = 4.5 m.
    let mut w = world(
        "spawn ball at=0,0.03,0\nrule on=start do=setvel:kind:ball:3,0,0\n",
        Some(flat()),
    );
    run(&mut w, 400);
    let (p, v) = ball(&w);
    assert!((p[0] - 4.5).abs() < 0.15, "rolled to {}", p[0]);
    assert_eq!(speed(v), 0.0);
    assert!((p[1] - 0.03).abs() < 1e-9, "stays on the ground");
}

#[test]
fn a_surface_changes_how_far_it_goes() {
    // x > 2 is sand (decel 8): it reaches x = 2 at sqrt(16 - 4) = 3.46 m/s and stops 12 / 16 = 0.75 m on.
    let sand = field(20.0, |_, _| 0.0, |x, _| u8::from(x > 2.0));
    let mut w = world(
        "surface sand parts=trap decel=8 bounce=0.1\nspawn ball at=0,0.03,0\nrule on=start do=setvel:kind:ball:4,0,0\n",
        Some(sand),
    );
    run(&mut w, 400);
    let p = ball(&w).0;
    assert!((p[0] - 2.75).abs() < 0.15, "stopped at {}", p[0]);
}

#[test]
fn a_steep_slope_pulls_a_ball_downhill_and_a_gentle_one_does_not() {
    // Slope 0.1 pulls with 5/7 * 9.8 * 0.0995 = 0.70 m/s^2 against a decel of 0.5: it rolls toward -x. Slope 0.03 pulls with 0.21: it stays.
    let steep = field(20.0, |x, _| (0.1 * x) as f32, |_, _| 1);
    let mut w = world(
        "surface green parts=g decel=0.5 bounce=0.5\nspawn ball at=0,0.03,0\n",
        Some(steep),
    );
    run(&mut w, 240);
    let (p, v) = ball(&w);
    assert!(p[0] < -0.8 && v[0] < -0.3, "rolled downhill: {p:?} {v:?}");
    assert!(
        (p[1] - (0.1 * p[0] + 0.03)).abs() < 0.01,
        "and follows the ground: {p:?}"
    );
    let gentle = field(20.0, |x, _| (0.03 * x) as f32, |_, _| 1);
    let mut w = world(
        "surface green parts=g decel=0.5 bounce=0.5\nspawn ball at=0,0.03,0\n",
        Some(gentle),
    );
    run(&mut w, 240);
    assert!(ball(&w).0[0].abs() < 0.01, "stays: {:?}", ball(&w).0);
}

#[test]
fn over_a_void_it_raises_out_once_and_falls() {
    let edge = field(20.0, |x, _| if x > 3.0 { f32::NAN } else { 0.0 }, |_, _| 1);
    let extra = "var outs=0\nsurface green parts=g decel=0.1 bounce=0.5\nspawn ball at=0,0.03,0\nrule on=start do=setvel:kind:ball:3,0,0\nrule on=out do=add:outs:1\n";
    let mut w = world(extra, Some(edge));
    run(&mut w, 240);
    assert_eq!(w.vars()["outs"], 1.0, "{:?}", w.vars());
    assert!(ball(&w).0[1] < -1.0, "it fell: {:?}", ball(&w).0);
}

#[test]
fn without_a_field_the_ground_line_is_the_ground() {
    let mut w = world("ground y=0.5\nspawn ball at=0,2,0\n", None);
    run(&mut w, 300);
    assert!((ball(&w).0[1] - 0.53).abs() < 1e-9, "{:?}", ball(&w).0);
}

// ---- the cup ----

const CUP: &str = "var sunk=0\nsurface green parts=g decel=0.1 bounce=0.5\nkind cup size=0.07 pocket=1\nspawn cup at=2,0,0\nspawn ball at=0,0.03,0\nrule on=pocket do=add:sunk:1;destroy:b\n";

fn greens() -> Arc<GroundField> {
    field(20.0, |_, _| 0.0, |_, _| 1)
}

#[test]
fn a_slow_ball_over_the_cup_sinks() {
    let mut w = world(
        &format!("{CUP}rule on=start do=setvel:kind:ball:1,0,0\n"),
        Some(greens()),
    );
    run(&mut w, 240);
    assert_eq!(w.vars()["sunk"], 1.0, "{:?}", w.vars());
    assert!(
        w.entities().iter().all(|e| e.kind != "ball"),
        "the rule took the ball"
    );
}

#[test]
fn a_fast_ball_runs_over_the_cup() {
    let mut w = world(
        &format!("{CUP}rule on=start do=setvel:kind:ball:4,0,0\n"),
        Some(greens()),
    );
    run(&mut w, 120);
    assert_eq!(w.vars()["sunk"], 0.0, "{:?}", w.vars());
    assert!(ball(&w).0[0] > 2.5, "it went past: {:?}", ball(&w).0);
}

// ---- the club ----

const CLUB: &str = "game name=golf\nvar strokes=0\nkind club size=0.05 tags=hand_r swing=1 reach=0.9 power=1.0\nkind loft_club size=0.05 tags=hand_r swing=1 reach=0.9 loft=45\nkind ball size=0.03 roll=1\ninput rh follow=right_hand target=tag:hand_r\nrule on=swing do=add:strokes:1\n";

fn club_world(club: &str, ball_at: &str) -> World {
    let mut w = World::new(
        &GameDef::parse(&format!(
            "{CLUB}spawn {club} at=0,0,0\nspawn ball at={ball_at}\n"
        ))
        .unwrap(),
        1,
    );
    w.set_ground(Some(flat()));
    w
}

fn hand(position: [f64; 3]) -> Inputs {
    Inputs {
        hands: vec![HandInput {
            side: HandSide::Right,
            position,
            grip: false,
            trigger: false,
        }],
        ..Inputs::default()
    }
}

/// Moves the hand along +X at `step` metres a step from `from` to `to`, at height 0.03 and z = 0 (the head is 0.9 m ahead along -Z).
fn swing(w: &mut World, from: f64, to: f64, step: f64) {
    let mut x = from;
    while x <= to + 1e-9 {
        w.step(&hand([x, 0.03, 0.0])).unwrap();
        x += step;
    }
}

#[test]
fn a_swing_through_the_ball_sends_it_off_with_the_heads_speed() {
    let mut w = club_world("club", "0,0.03,-0.9");
    swing(&mut w, -0.5, 0.5, 0.05); // 3 m/s
    let (_, v) = ball(&w);
    assert!(
        (v[0] - 3.0).abs() < 0.4 && v[2].abs() < 0.25 && v[1] == 0.0,
        "{v:?}"
    );
    assert_eq!(w.vars()["strokes"], 1.0);
}

#[test]
fn a_very_fast_head_does_not_pass_through_the_ball() {
    // 30 m/s is half a metre a step; the head is at x = 0 and then x = 0.5 either side of a ball at x = 0.25.
    let mut w = club_world("club", "0.25,0.03,-0.9");
    swing(&mut w, -1.0, 1.0, 0.5);
    let (_, v) = ball(&w);
    assert!(v[0] > 25.0 && v[0] < 33.0, "{v:?}");
    assert_eq!(w.vars()["strokes"], 1.0);
}

#[test]
fn the_ball_speed_is_capped() {
    let mut w = club_world("club", "0.5,0.03,-0.9");
    swing(&mut w, -1.0, 2.0, 1.0); // 60 m/s
    assert!((speed(ball(&w).1) - 45.0).abs() < 1.0, "{:?}", ball(&w).1);
}

#[test]
fn one_swing_is_one_stroke_however_long_the_head_stays_with_the_ball() {
    let mut w = club_world("club", "0,0.03,-0.9");
    swing(&mut w, -0.5, 1.5, 0.05); // the ball runs ahead at the head's own speed
    assert_eq!(w.vars()["strokes"], 1.0, "{:?}", w.vars());
}

#[test]
fn a_slow_wave_is_not_a_stroke() {
    let mut w = club_world("club", "0,0.03,-0.9");
    swing(&mut w, -0.2, 0.2, 0.004); // 0.24 m/s
    assert_eq!(w.vars()["strokes"], 0.0);
    assert_eq!(speed(ball(&w).1), 0.0);
}

#[test]
fn a_lofted_club_lifts_the_ball_and_a_putter_keeps_it_down() {
    let mut w = club_world("loft_club", "0,0.03,-0.9");
    // The velocity is read on the step of the hit: a step later gravity has begun to take it back down.
    let mut x = -0.5;
    let mut v = [0.0; 3];
    while x <= 0.5 + 1e-9 && w.vars()["strokes"] < 1.0 {
        w.step(&hand([x, 0.03, 0.0])).unwrap();
        v = ball(&w).1;
        x += 0.05;
    }
    assert!(
        (v[0] - 2.12).abs() < 0.3 && (v[1] - 2.12).abs() < 0.3,
        "45 degrees: {v:?}"
    );
}

#[test]
fn the_hands_rotation_turns_the_head() {
    // The hand turned a quarter turn about +Y points its -Z axis along -X: the head is 0.9 m out toward -X.
    let q = [0.0, FRAC_1_SQRT_2, 0.0, FRAC_1_SQRT_2];
    let mut w = club_world("club", "-0.9,0.03,0");
    for i in 0..24 {
        let z = -0.5 + f64::from(i) * 0.05; // sweeps the head along +Z through the ball at z = 0
        let mut input = hand([0.0, 0.03, z]);
        input.hand_rotations = vec![HandRotation {
            side: HandSide::Right,
            rotation: q,
        }];
        w.step(&input).unwrap();
    }
    let v = ball(&w).1;
    assert!(v[2] > 2.0 && v[0].abs() < 0.4, "hit along +Z: {v:?}");
    let club = w.entities().iter().find(|e| e.kind == "club").unwrap();
    for (a, b) in club.orientation.iter().zip(q) {
        assert!((a - b).abs() < 1e-9, "{:?}", club.orientation);
    }
}

// ---- text and determinism ----

#[test]
fn the_new_lines_and_keys_round_trip() {
    let text = format!(
        "{CLUB}surface sand parts=trap,bunker decel=8 bounce=0.1\nterrain parts=ground,fairway cell=0.2\nkind cup size=0.07 pocket=1\nspawn ball at=0,0,0\n"
    );
    let def = GameDef::parse(&text).unwrap();
    let described = def.describe();
    for needle in [
        "swing=1",
        "reach=0.9",
        "loft=45",
        "roll=1",
        "pocket=1",
        "surface sand parts=trap,bunker decel=8 bounce=0.1",
        "terrain parts=ground,fairway cell=0.2",
    ] {
        assert!(described.contains(needle), "no `{needle}` in {described}");
    }
    assert_eq!(GameDef::parse(&described).unwrap().describe(), described);
}

#[test]
fn bad_rolling_text_names_the_key() {
    for (line, key) in [
        ("surface sand parts=trap decel=-1\n", "decel"),
        ("surface sand decel=2\n", "parts"),
        ("terrain cell=0.2\n", "parts"),
        ("terrain parts=g cell=0\n", "cell"),
        ("kind a size=0.1 roll=maybe\n", "roll"),
        ("kind a size=0.1 swing=1 reach=-1\n", "reach"),
        ("kind a size=0.1 swing=1 loft=100\n", "loft"),
        ("kind a size=0.1 swing=1 power=0\n", "power"),
        ("kind a size=0.1 pocket=2\n", "pocket"),
    ] {
        let diagnostics = GameDef::parse(&format!("game name=a\n{line}")).unwrap_err();
        assert!(
            diagnostics.iter().any(|d| d.message.contains(key)),
            "{line}: {diagnostics:?}"
        );
    }
}

#[test]
fn the_same_round_gives_the_same_hash() {
    let make = || {
        let mut w = club_world("club", "0.25,0.03,-0.9");
        swing(&mut w, -1.0, 1.0, 0.25);
        run(&mut w, 120);
        w.hash()
    };
    assert_eq!(make(), make());
}

#[test]
fn a_game_without_rolling_things_hashes_as_before() {
    let text = "game name=a\nkind coin size=0.4\nspawn coin at=1,0,0\n";
    let mut with = World::new(&GameDef::parse(text).unwrap(), 1);
    with.set_ground(Some(flat()));
    let mut without = World::new(&GameDef::parse(text).unwrap(), 1);
    for _ in 0..30 {
        with.step(&Inputs::default()).unwrap();
        without.step(&Inputs::default()).unwrap();
    }
    assert_eq!(with.hash(), without.hash());
}
