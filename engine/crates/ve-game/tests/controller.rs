//! Character controllers (F1 of `plans/game-foundations-plan.md`): a kind declared with a `controller` line moves from named input axes with
//! real dynamics (acceleration, deceleration, air control), gravity and jumping (variable height, extra jumps, coyote time, jump buffer),
//! lands on the ground and on the tops of `solid` boxes, can move relative to the camera, turns at a set rate, and is deterministic.
//! Kinds without a controller move exactly as before. The entity's y is its feet. Written before the body; do not weaken.
#![expect(clippy::unwrap_used, clippy::float_cmp, reason = "fixtures are known to be valid and compare exact rest values")]
use ve_game::{GameDef, Inputs, World};

const DT: f64 = 1.0 / 60.0;

const BASE: &str = "game name=ctl
kind hero size=0.4 speed=5 tags=hero
controller hero mode=walk speed=5 accel=50 decel=50 air=0.5 jump=8 gravity=20 coyote=0.1 buffer=0.1 extra_jumps=0 cut=0.4
input move axis=x,z target=tag:hero
input jump key=press
spawn hero at=0,0,0
";

fn game(extra: &str) -> World {
    World::new(&GameDef::parse(&format!("{BASE}{extra}")).unwrap(), 1)
}

fn controlled(controller: &str, spawn: &str, extra: &str) -> World {
    let text = format!(
        "game name=ctl\nkind hero size=0.4 speed=5 tags=hero\n{extra}\ncontroller hero {controller}\ninput move axis=x,z target=tag:hero\ninput jump key=press\nspawn hero at={spawn}\n"
    );
    World::new(&GameDef::parse(&text).unwrap(), 1)
}

fn press(world: &mut World, move_x: f64, move_z: f64, jump: bool, camera_yaw: Option<f64>) {
    let mut inputs = Inputs::default();
    inputs.axes.insert("move.x".into(), move_x);
    inputs.axes.insert("move.z".into(), move_z);
    if let Some(yaw) = camera_yaw {
        inputs.axes.insert("camera.yaw".into(), yaw);
    }
    if jump {
        inputs.pressed.push("jump".into());
    }
    world.step(&inputs).unwrap();
}

fn run(world: &mut World, steps: usize, move_x: f64, move_z: f64) {
    for _ in 0..steps {
        press(world, move_x, move_z, false, None);
    }
}

fn hero(world: &World) -> &ve_game::Entity {
    world.entities().iter().find(|e| e.kind == "hero").unwrap()
}

fn speed(world: &World) -> f64 {
    hero(world).props.get("speed").copied().unwrap_or(f64::NAN)
}

fn grounded(world: &World) -> bool {
    hero(world).props.get("grounded").copied() == Some(1.0)
}

#[test]
fn speed_ramps_up_at_the_set_acceleration_and_caps_at_the_set_speed() {
    let mut world = game("");
    run(&mut world, 3, 1.0, 0.0);
    assert!((speed(&world) - 2.5).abs() < 0.1, "50 m/s2 for 3/60 s is 2.5 m/s, got {}", speed(&world));
    for _ in 0..60 {
        run(&mut world, 1, 1.0, 0.0);
        assert!(speed(&world) <= 5.0 + 1e-9, "never above the set speed: {}", speed(&world));
    }
    assert!((speed(&world) - 5.0).abs() < 1e-6);
    assert!(hero(&world).position[0] > 3.0, "it has travelled: {:?}", hero(&world).position);
}

#[test]
fn releasing_the_stick_slows_to_a_stop_at_the_set_deceleration() {
    let mut world = game("");
    run(&mut world, 40, 1.0, 0.0);
    run(&mut world, 3, 0.0, 0.0);
    assert!((speed(&world) - 2.5).abs() < 0.1, "5 m/s minus 50 m/s2 for 3/60 s: {}", speed(&world));
    run(&mut world, 10, 0.0, 0.0);
    assert_eq!(speed(&world), 0.0);
    let rest = hero(&world).position;
    run(&mut world, 10, 0.0, 0.0);
    assert_eq!(hero(&world).position, rest, "it stays where it stopped");
}

#[test]
fn diagonal_input_is_no_faster_than_straight_input() {
    let mut world = game("");
    run(&mut world, 60, 1.0, 1.0);
    assert!(speed(&world) <= 5.0 + 1e-9, "{}", speed(&world));
    assert!(speed(&world) > 4.9);
}

#[test]
fn a_jump_rises_as_high_as_speed_and_gravity_say_and_comes_back_down() {
    let mut world = game("");
    run(&mut world, 2, 0.0, 0.0);
    assert!(grounded(&world));
    let (mut apex, mut airborne) = (0.0_f64, 0);
    for step in 0..120 {
        press(&mut world, 0.0, 0.0, true, None);
        if step > 0 && !grounded(&world) {
            airborne += 1;
        }
        apex = apex.max(hero(&world).position[1]);
        if step > 5 && grounded(&world) {
            break;
        }
    }
    // v squared over 2g = 64 / 40 = 1.6 m; flight 2v/g = 0.8 s = 48 steps
    assert!((apex - 1.6).abs() < 0.12, "apex {apex}");
    assert!((44..=52).contains(&airborne), "airborne for {airborne} steps");
    assert!(grounded(&world));
    assert!(hero(&world).position[1].abs() < 1e-9, "back on the ground: {}", hero(&world).position[1]);
}

#[test]
fn releasing_the_jump_early_cuts_the_jump_short() {
    let apex = |hold: usize| {
        let mut world = game("");
        run(&mut world, 2, 0.0, 0.0);
        let mut top = 0.0_f64;
        for step in 0..120 {
            press(&mut world, 0.0, 0.0, step < hold, None);
            top = top.max(hero(&world).position[1]);
            if step > 5 && grounded(&world) {
                break;
            }
        }
        top
    };
    let (full, tap) = (apex(100), apex(3));
    assert!(tap < full * 0.7, "a tap jumps lower than a held jump: {tap} against {full}");
    assert!(tap > 0.1, "but it is still a jump: {tap}");
}

#[test]
fn a_second_jump_in_the_air_needs_extra_jumps_and_works_once_when_given() {
    let apex = |controller: &str| {
        let mut world = controlled(controller, "0,0,0", "");
        run(&mut world, 2, 0.0, 0.0);
        let mut top = 0.0_f64;
        for step in 0..150 {
            // press and hold for a while, let go, then press again in the air at step 20, and again at step 35
            let held = step < 15 || (20..32).contains(&step) || (35..45).contains(&step);
            press(&mut world, 0.0, 0.0, held, None);
            top = top.max(hero(&world).position[1]);
        }
        top
    };
    // (cut=1: releasing the button early keeps the whole jump, so only the extra jumps change the height)
    let single = apex("speed=5 accel=50 decel=50 jump=8 gravity=20 cut=1 extra_jumps=0");
    let double = apex("speed=5 accel=50 decel=50 jump=8 gravity=20 cut=1 extra_jumps=1");
    assert!((single - 1.6).abs() < 0.15, "without extra jumps the air presses do nothing: {single}");
    assert!(double > 2.4, "one extra jump adds height: {double}");
    let triple = apex("speed=5 accel=50 decel=50 jump=8 gravity=20 cut=1 extra_jumps=2");
    assert!(triple > double + 0.3, "two extra jumps add more: {triple} against {double}");
}

/// A 4 x 1 x 4 block centred at the origin: its top is at y = 1 and its edge at x = 2.
const BLOCK: &str = "kind block size=0.1 solid=4,1,4\nspawn block at=0,0.5,0\n";

#[test]
fn a_solid_box_is_a_platform_to_land_on_and_to_walk_off() {
    let mut world = controlled("speed=5 accel=50 decel=50 jump=8 gravity=20", "0,3,0", BLOCK);
    run(&mut world, 60, 0.0, 0.0);
    assert!((hero(&world).position[1] - 1.0).abs() < 1e-6, "it lands on the top: {}", hero(&world).position[1]);
    assert!(grounded(&world));
    run(&mut world, 90, 1.0, 0.0);
    assert!(hero(&world).position[0] > 2.5, "it walked off the edge: {:?}", hero(&world).position);
    assert!(hero(&world).position[1].abs() < 1e-6, "and fell to the ground: {}", hero(&world).position[1]);
}

#[test]
fn coyote_time_lets_a_jump_start_just_after_walking_off_a_ledge() {
    let rising_after = |wait: usize| {
        let mut world = controlled("speed=5 accel=50 decel=50 jump=8 gravity=20 coyote=0.1 buffer=0.1", "1.5,1,0", BLOCK);
        run(&mut world, 3, 0.0, 0.0);
        let mut left = None;
        for step in 0..120 {
            press(&mut world, 1.0, 0.0, false, None);
            if left.is_none() && !grounded(&world) {
                left = Some(step);
                break;
            }
        }
        assert!(left.is_some(), "it did walk off");
        run(&mut world, wait, 1.0, 0.0);
        press(&mut world, 1.0, 0.0, true, None);
        hero(&world).velocity[1] > 0.0
    };
    assert!(rising_after(2), "two steps after the edge is inside the 0.1 s window");
    assert!(!rising_after(12), "twelve steps after the edge is outside it");
}

#[test]
fn a_jump_pressed_just_before_landing_fires_on_landing() {
    let rejumps = |before_landing: usize| {
        let mut world = game("");
        run(&mut world, 2, 0.0, 0.0);
        press(&mut world, 0.0, 0.0, true, None);
        let mut steps_to_land: usize = 0;
        while !grounded(&world) && steps_to_land < 200 {
            press(&mut world, 0.0, 0.0, false, None);
            steps_to_land += 1;
        }
        // go round again, pressing `before_landing` steps before the landing step
        let mut world = game("");
        run(&mut world, 2, 0.0, 0.0);
        press(&mut world, 0.0, 0.0, true, None);
        for step in 1..=steps_to_land {
            let press_now = steps_to_land.saturating_sub(step) + 1 == before_landing;
            press(&mut world, 0.0, 0.0, press_now, None);
        }
        hero(&world).velocity[1] > 0.0
    };
    assert!(rejumps(4), "pressed 4 steps early (inside the 6 step buffer) it jumps the moment it lands");
    assert!(!rejumps(14), "pressed 14 steps early (outside the buffer) it does not");
}

#[test]
fn camera_relative_moves_follow_the_camera_yaw() {
    let travel = |yaw: f64, x: f64, z: f64| {
        let mut world = controlled("speed=5 accel=500 decel=500 relative=camera", "0,0,0", "");
        for _ in 0..30 {
            press(&mut world, x, z, false, Some(yaw));
        }
        hero(&world).position
    };
    let half_pi = std::f64::consts::FRAC_PI_2;
    let p = travel(0.0, 0.0, 1.0);
    assert!(p[2] > 1.0 && p[0].abs() < 1e-6, "facing +z, forward is +z: {p:?}");
    let p = travel(half_pi, 0.0, 1.0);
    assert!(p[0] > 1.0 && p[2].abs() < 1e-6, "facing +x, forward is +x: {p:?}");
    let p = travel(half_pi, 1.0, 0.0);
    assert!(p[2] > 1.0 && p[0].abs() < 1e-6, "facing +x, right is +z: {p:?}");
    let p = travel(std::f64::consts::PI, 1.0, 0.0);
    assert!(p[0] > 1.0 && p[2].abs() < 1e-6, "facing -z, right is +x: {p:?}");
}

#[test]
fn a_locked_axis_confines_a_platformer_to_a_plane() {
    let mut world = controlled("mode=platformer speed=5 accel=50 decel=50 jump=8 gravity=20 lock=z", "0,0,0", "");
    run(&mut world, 30, 1.0, 1.0);
    assert!(hero(&world).position[0] > 1.0, "it moves along x");
    assert_eq!(hero(&world).position[2], 0.0, "and never along z");
}

#[test]
fn the_hero_turns_toward_its_motion_at_the_set_turn_rate() {
    let mut world = controlled("speed=5 accel=500 decel=500 turn=360", "0,0,0", "");
    run(&mut world, 5, 1.0, 0.0);
    let after_five = hero(&world).heading;
    assert!((after_five - 360.0_f64.to_radians() * 5.0 * DT).abs() < 0.08, "30 degrees in 5 steps: {after_five}");
    run(&mut world, 30, 1.0, 0.0);
    assert!((hero(&world).heading - std::f64::consts::FRAC_PI_2).abs() < 1e-6, "it ends facing +x: {}", hero(&world).heading);
}

#[test]
fn the_same_inputs_give_the_same_world() {
    let play = || {
        let mut world = game(BLOCK);
        for step in 0..600 {
            let (x, z) = (f64::from((step / 50) % 3) - 1.0, f64::from((step / 70) % 3) - 1.0);
            press(&mut world, x, z, step % 90 < 8, None);
        }
        (world.hash(), hero(&world).position)
    };
    assert_eq!(play(), play());
}

#[test]
fn kinds_without_a_controller_move_exactly_as_before() {
    let text = "game name=old\nkind walker size=0.4 speed=2 tags=hero\ninput move axis=x,z target=tag:hero\nspawn walker at=0,0,0\n";
    let mut world = World::new(&GameDef::parse(text).unwrap(), 1);
    for _ in 0..30 {
        let mut inputs = Inputs::default();
        inputs.axes.insert("move.x".into(), 1.0);
        world.step(&inputs).unwrap();
    }
    assert!((world.entities()[0].position[0] - 2.0 * 30.0 * DT).abs() < 1e-9, "speed times time, at once: {:?}", world.entities()[0].position);
}

#[test]
fn bad_controllers_are_refused_with_a_reason() {
    let refused = |line: &str| {
        let text = format!("game name=ctl\nkind hero size=0.4 speed=5 tags=hero\n{line}\n");
        GameDef::parse(&text).unwrap_err().into_iter().map(|d| d.message).collect::<Vec<_>>().join(" | ")
    };
    assert!(refused("controller ghost speed=5").contains("ghost"), "an unknown kind is named");
    assert!(refused("controller hero mode=swim").contains("mode"), "an unknown mode");
    assert!(refused("controller hero speed=-1").contains("speed"));
    assert!(refused("controller hero jump=-3").contains("jump"));
    assert!(refused("controller hero coyote=-1").contains("coyote"));
    assert!(refused("controller hero lock=y").contains("lock"));
    assert!(refused("controller hero relative=moon").contains("relative"));
    assert!(refused("controller hero speed=5\ncontroller hero speed=6").contains("duplicate"));
}

#[test]
fn a_controller_survives_a_text_round_trip() {
    let def = GameDef::parse(BASE).unwrap();
    let again = GameDef::parse(&def.to_text()).unwrap();
    assert_eq!(def.to_text(), again.to_text());
    assert!(def.to_text().contains("controller hero"), "the line is kept");
}

#[test]
fn falling_below_the_limit_returns_the_hero_to_its_last_safe_ground_and_raises_fell() {
    let mut world = controlled(
        "mode=walk speed=5 accel=50 decel=50 fall_limit=-6",
        "0,0,0",
        "ground y=-20\nvar falls=0\nkind slab size=1 solid=6,1,6\nspawn slab at=0,-0.5,0\nrule on=fell a=tag:hero do=add:falls:1",
    );
    // stand still a moment so a safe place is recorded, then walk off the edge of the slab (it ends at x = 3)
    run(&mut world, 60, 0.0, 0.0);
    for _ in 0..240 {
        press(&mut world, 1.0, 0.0, false, None);
        if world.vars().get("falls").copied().unwrap_or(0.0) > 0.0 {
            break;
        }
    }
    assert_eq!(world.vars().get("falls").copied(), Some(1.0), "a fell event reached the rule");
    let at = hero(&world);
    assert!(at.position[1] > -1.0 && at.position[0] < 3.0, "back on the slab: {:?}", at.position);
    assert_eq!(at.velocity, [0.0, 0.0, 0.0], "at rest");
}

#[test]
fn walking_diagonally_into_a_wall_slides_along_it() {
    let mut world = controlled(
        "mode=walk speed=5 accel=50 decel=50",
        "0,0,0",
        "kind wall size=1 solid=1,3,40\nspawn wall at=2,1.5,0",
    );
    for _ in 0..120 {
        press(&mut world, 1.0, 1.0, false, None);
    }
    let at = hero(&world).position;
    assert!(at[0] < 1.3 && at[0] > 1.0, "stopped against the wall face at x = 1.5 minus the radius: {at:?}");
    assert!(at[2].abs() > 3.0, "and slid along it: {at:?}");
}

#[test]
fn a_controlled_hero_feeds_the_animation_machine_when_things_change() {
    use ve_game::EffectKind;
    let mut world = game("");
    let mut seen: Vec<(EffectKind, String, Option<f64>)> = Vec::new();
    let step = |world: &mut World, move_z: f64, jump: bool| {
        let mut inputs = Inputs::default();
        inputs.axes.insert("move.z".into(), move_z);
        if jump {
            inputs.pressed.push("jump".into());
            inputs.axes.insert("jump".into(), 1.0);
        }
        world.step(&inputs).unwrap().effects.into_iter().map(|e| (e.kind, e.name, e.value)).collect::<Vec<_>>()
    };
    for _ in 0..90 {
        seen.extend(step(&mut world, 1.0, false));
    }
    let speeds: Vec<f64> = seen.iter().filter(|(k, n, _)| *k == EffectKind::AnimSet && n == "speed").filter_map(|(_, _, v)| *v).collect();
    assert!(speeds.len() >= 3 && speeds.len() < 40, "speed is sent as it changes, not every step: {} updates", speeds.len());
    assert!(speeds.windows(2).all(|w| w[1] >= w[0]), "and it rises while accelerating: {speeds:?}");
    assert!((speeds.last().unwrap() - 5.0).abs() < 0.11, "to the top speed: {speeds:?}");
    seen.clear();
    seen.extend(step(&mut world, 1.0, true));
    for _ in 0..90 {
        seen.extend(step(&mut world, 1.0, false));
    }
    let named = |kind: EffectKind, name: &str| seen.iter().filter(|(k, n, _)| *k == kind && n == name).count();
    assert_eq!(named(EffectKind::Anim, "jump"), 1, "one jump trigger: {seen:?}");
    assert_eq!(named(EffectKind::Anim, "land"), 1, "one landing trigger");
    let flags: Vec<f64> = seen.iter().filter(|(k, n, _)| *k == EffectKind::AnimFlag && n == "grounded").filter_map(|(_, _, v)| *v).collect();
    assert_eq!(flags, vec![0.0, 1.0], "grounded goes false on take-off and true on landing");
}

#[test]
fn a_held_axis_keeps_the_jump_full_and_a_lone_press_is_cut() {
    // The page sends a press on key down and an axis of the same name while the key is held; a host with only presses is a short hop.
    let peak = |held_steps: usize| {
        let mut world = game("");
        let mut best: f64 = 0.0;
        for step in 0..60 {
            let mut inputs = Inputs::default();
            if step == 0 {
                inputs.pressed.push("jump".into());
            }
            if step < held_steps {
                inputs.axes.insert("jump".into(), 1.0);
            }
            world.step(&inputs).unwrap();
            best = best.max(hero(&world).position[1]);
        }
        best
    };
    let held = peak(30);
    let tapped = peak(0);
    assert!(held > 1.5, "held through the rise, the full jump (8^2/(2*20) = 1.6 m): {held}");
    assert!(tapped < 0.6 * held, "released at once, it is cut (cut=0.4 keeps 16 percent of the height): {tapped}");
}
