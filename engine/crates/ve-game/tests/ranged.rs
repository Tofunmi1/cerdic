//! Ranged combat and health (F2 of `plans/game-foundations-plan.md`): `health KIND hp=... iframes=... knock=...` gives entities hit points,
//! `hurt` and `heal` change them (raising `hurt` and `killed`), `projectile NAME ...` describes a flying shot, `weapon NAME ...` fires one (or
//! a hitscan ray) with ammo, reload, cooldown and spread, aimed along the owner's heading, the camera or at a target. Pure and deterministic.
//! Written before the body; do not weaken.
#![expect(clippy::unwrap_used, clippy::float_cmp, reason = "fixtures are known to be valid and compare exact health and ammo counts")]
use ve_game::{GameDef, Inputs, World};

const BASE: &str = "game name=ranged
var kills=0
var hurts=0
var impacts=0
var empties=0
kind hero size=0.4 tags=hero speed=0
kind dummy size=0.5 tags=enemy
kind bullet size=0.1 tags=shot
kind wall size=1 solid=6,4,1
input fire key=press
input reload key=press
health hero hp=100
health dummy hp=30 height=1.8
projectile bullet speed=20 life=2 damage=10
weapon rifle mode=projectile shot=bullet input=fire reload_input=reload owner=tag:hero ammo=3 reload=1 cooldown=0.25 aim=heading muzzle=1.2
spawn hero at=0,0,0
rule on=hurt a=tag:enemy do=add:hurts:1
rule on=killed a=tag:enemy do=add:kills:1
rule on=impact do=add:impacts:1
rule on=empty do=add:empties:1
";

fn world(extra: &str) -> World {
    World::new(&GameDef::parse(&format!("{BASE}{extra}")).unwrap(), 5)
}

fn step(world: &mut World, pressed: &[&str]) {
    let inputs = Inputs { pressed: pressed.iter().map(|p| (*p).to_owned()).collect(), ..Inputs::default() };
    world.step(&inputs).unwrap();
}

fn run(world: &mut World, steps: usize) {
    for _ in 0..steps {
        step(world, &[]);
    }
}

fn var(world: &World, name: &str) -> f64 {
    world.vars().get(name).copied().unwrap_or(f64::NAN)
}

fn prop(world: &World, kind: &str, name: &str) -> Option<f64> {
    world.entities().iter().find(|e| e.kind == kind).and_then(|e| e.props.get(name).copied())
}

fn count(world: &World, kind: &str) -> usize {
    world.entities().iter().filter(|e| e.kind == kind).count()
}

fn near(a: f64, b: f64, tolerance: f64, what: &str) {
    assert!((a - b).abs() <= tolerance, "{what}: {a} against {b}");
}

#[test]
fn health_comes_from_the_kind_and_hurt_and_heal_change_it() {
    let mut w = world("spawn dummy at=0,0,5\ninput hit key=press\nrule on=input:hit do=hurt:kind:dummy:12\ninput mend key=press\nrule on=input:mend do=heal:kind:dummy:100\n");
    assert_eq!(prop(&w, "dummy", "health"), Some(30.0));
    assert_eq!(prop(&w, "dummy", "max_health"), Some(30.0));
    step(&mut w, &["hit"]);
    assert_eq!(prop(&w, "dummy", "health"), Some(18.0));
    assert_eq!(var(&w, "hurts"), 1.0, "the hurt event was raised");
    step(&mut w, &["mend"]);
    assert_eq!(prop(&w, "dummy", "health"), Some(30.0), "healing stops at the maximum");
}

#[test]
fn reaching_zero_raises_killed_and_removes_the_entity() {
    let mut w = world("spawn dummy at=0,0,5\ninput hit key=press\nrule on=input:hit do=hurt:kind:dummy:20\n");
    step(&mut w, &["hit"]);
    step(&mut w, &["hit"]);
    assert_eq!(var(&w, "kills"), 1.0);
    assert_eq!(count(&w, "dummy"), 0, "a dead entity is gone");
    assert_eq!(var(&w, "hurts"), 2.0, "the killing blow was also a hurt");
}

#[test]
fn invulnerability_frames_ignore_a_second_hit_until_they_end() {
    let mut w = world("health dummy hp=30 iframes=0.5\nspawn dummy at=0,0,5\ninput hit key=press\nrule on=input:hit do=hurt:kind:dummy:5\n");
    step(&mut w, &["hit"]);
    step(&mut w, &["hit"]);
    assert_eq!(prop(&w, "dummy", "health"), Some(25.0), "the second hit fell inside the window");
    run(&mut w, 40);
    step(&mut w, &["hit"]);
    assert_eq!(prop(&w, "dummy", "health"), Some(20.0), "after half a second it hurts again");
    assert_eq!(var(&w, "hurts"), 2.0);
}

#[test]
fn knockback_pushes_the_victim_away_from_the_source() {
    let mut w = world("health dummy hp=30 knock=4 height=1.8\nspawn dummy at=2,0,5\ninput hit key=press\nrule on=input:hit do=hurt:kind:dummy:1@kind:hero\n");
    step(&mut w, &["hit"]);
    let dummy = w.entities().iter().find(|e| e.kind == "dummy").unwrap();
    assert!(dummy.velocity[0] > 0.5 && dummy.velocity[2] > 0.5, "pushed away from the hero at the origin: {:?}", dummy.velocity);
}

#[test]
fn firing_spawns_a_shot_at_the_muzzle_that_flies_along_the_heading() {
    let mut w = world("");
    step(&mut w, &["fire"]);
    assert_eq!(count(&w, "bullet"), 1);
    let shot = w.entities().iter().find(|e| e.kind == "bullet").unwrap().clone();
    near(shot.position[1], 1.2, 1e-9, "muzzle height");
    assert!(shot.position[2] > 0.0 && shot.position[0].abs() < 1e-9, "ahead of the hero along +z: {:?}", shot.position);
    let before = shot.position[2];
    run(&mut w, 30);
    let after = w.entities().iter().find(|e| e.kind == "bullet").unwrap().position[2];
    near(after - before, 20.0 * 0.5, 0.2, "twenty metres a second for half a second");
    assert_eq!(prop(&w, "hero", "ammo_rifle"), Some(2.0), "one round used");
}

#[test]
fn a_shot_hits_a_victim_in_its_path_hurts_it_and_is_used_up() {
    let mut w = world("spawn dummy at=0,0,6\n");
    step(&mut w, &["fire"]);
    run(&mut w, 40);
    assert_eq!(prop(&w, "dummy", "health"), Some(20.0), "ten damage");
    assert_eq!(count(&w, "bullet"), 0, "the shot is gone");
    assert_eq!(var(&w, "hurts"), 1.0);
}

#[test]
fn a_shot_that_misses_flies_on_and_expires_with_an_impact() {
    let mut w = world("spawn dummy at=5,0,6\n");
    step(&mut w, &["fire"]);
    run(&mut w, 180);
    assert_eq!(prop(&w, "dummy", "health"), Some(30.0));
    assert_eq!(count(&w, "bullet"), 0, "after two seconds");
    assert_eq!(var(&w, "impacts"), 1.0);
}

#[test]
fn the_shooter_is_never_hit_by_its_own_shot() {
    let mut w = world("projectile bullet speed=20 life=2 damage=10 gravity=30\n");
    step(&mut w, &["fire"]);
    run(&mut w, 120);
    assert_eq!(prop(&w, "hero", "health"), Some(100.0), "the hero walks under its own falling shot unharmed");
}

#[test]
fn gravity_bends_the_flight_downward() {
    let mut w = world("projectile bullet speed=10 life=3 damage=10 gravity=9.8\n");
    step(&mut w, &["fire"]);
    run(&mut w, 30);
    let shot = w.entities().iter().find(|e| e.kind == "bullet").unwrap();
    assert!(shot.position[1] < 1.2 - 0.9, "fell about 1.2 m in half a second: {:?}", shot.position);
}

#[test]
fn a_wall_stops_a_shot() {
    let mut w = world("spawn wall at=0,2,4\nspawn dummy at=0,0,8\n");
    step(&mut w, &["fire"]);
    run(&mut w, 60);
    assert_eq!(prop(&w, "dummy", "health"), Some(30.0), "behind the wall, untouched");
    assert_eq!(count(&w, "bullet"), 0);
    assert_eq!(var(&w, "impacts"), 1.0, "it hit the wall");
}

#[test]
fn a_piercing_shot_goes_through_and_hits_both() {
    let mut w = world("projectile bullet speed=20 life=2 damage=10 pierce=1\nspawn dummy at=0,0,5\nspawn dummy at=0,0,8\nspawn dummy at=0,0,11\n");
    step(&mut w, &["fire"]);
    run(&mut w, 90);
    let healths: Vec<f64> = w.entities().iter().filter(|e| e.kind == "dummy").map(|e| e.props["health"]).collect();
    assert_eq!(healths, vec![20.0, 20.0, 30.0], "pierce=1 means one extra victim");
}

#[test]
fn splash_hurts_the_neighbours_less_with_distance() {
    let mut w = world("projectile bullet speed=20 life=2 damage=20 splash=4\nspawn dummy at=0,0,6\nspawn dummy at=2,0,6\nspawn dummy at=9,0,6\n");
    step(&mut w, &["fire"]);
    run(&mut w, 60);
    let healths: Vec<f64> = w.entities().iter().filter(|e| e.kind == "dummy").map(|e| e.props["health"]).collect();
    assert!(healths[0] < healths[1] && healths[1] < 30.0, "the nearer takes more: {healths:?}");
    assert_eq!(healths[2], 30.0, "outside the splash radius");
}

#[test]
fn a_homing_shot_curves_toward_a_victim_off_its_line() {
    let mut w = world("projectile bullet speed=12 life=3 damage=10 homing=240\nspawn dummy at=4,0,8\n");
    step(&mut w, &["fire"]);
    run(&mut w, 120);
    assert_eq!(prop(&w, "dummy", "health"), Some(20.0), "it turned and hit");
}

#[test]
fn a_hitscan_weapon_hits_the_nearest_victim_at_once() {
    let text = format!("{BASE}weapon laser mode=hitscan range=30 damage=15 input=zap owner=tag:hero aim=heading muzzle=1.0\ninput zap key=press\nspawn dummy at=0,0,6\nspawn dummy at=0,0,12\n");
    let mut w = World::new(&GameDef::parse(&text).unwrap(), 5);
    step(&mut w, &["zap"]);
    let healths: Vec<f64> = w.entities().iter().filter(|e| e.kind == "dummy").map(|e| e.props["health"]).collect();
    assert_eq!(healths, vec![15.0, 30.0], "only the nearer one, in the same step");
}

#[test]
fn hitscan_is_blocked_by_walls_and_limited_by_range() {
    let base = format!("{BASE}weapon laser mode=hitscan range=10 damage=15 input=zap owner=tag:hero aim=heading muzzle=1.0\ninput zap key=press\n");
    let mut walled = World::new(&GameDef::parse(&format!("{base}spawn wall at=0,2,3\nspawn dummy at=0,0,6\n")).unwrap(), 5);
    step(&mut walled, &["zap"]);
    assert_eq!(prop(&walled, "dummy", "health"), Some(30.0), "the wall is in the way");
    let mut far = World::new(&GameDef::parse(&format!("{base}spawn dummy at=0,0,14\n")).unwrap(), 5);
    step(&mut far, &["zap"]);
    assert_eq!(prop(&far, "dummy", "health"), Some(30.0), "beyond range 10");
}

#[test]
fn ammo_runs_out_reloading_refills_and_an_empty_trigger_raises_empty() {
    let mut w = world("");
    for _ in 0..3 {
        step(&mut w, &["fire"]);
        run(&mut w, 20);
    }
    assert_eq!(prop(&w, "hero", "ammo_rifle"), Some(0.0));
    step(&mut w, &["fire"]);
    assert_eq!(var(&w, "empties"), 1.0, "a click on an empty magazine");
    assert_eq!(count(&w, "bullet"), 3, "no fourth shot");
    step(&mut w, &["reload"]);
    run(&mut w, 30);
    assert_eq!(prop(&w, "hero", "ammo_rifle"), Some(0.0), "half a second in, still reloading");
    step(&mut w, &["fire"]);
    assert_eq!(count(&w, "bullet"), 3, "cannot fire while reloading");
    run(&mut w, 40);
    assert_eq!(prop(&w, "hero", "ammo_rifle"), Some(3.0), "a second later the magazine is full");
}

#[test]
fn the_cooldown_limits_the_rate_of_fire() {
    let mut w = world("projectile bullet speed=20 life=5 damage=10\n");
    for _ in 0..30 {
        step(&mut w, &["fire"]);
    }
    assert_eq!(count(&w, "bullet"), 2, "three rounds and a quarter-second cooldown: at step 0 and step 15 within 30 steps");
}

#[test]
fn spread_scatters_the_shots_the_same_way_every_time() {
    let spread = "weapon rifle mode=projectile shot=bullet input=fire owner=tag:hero ammo=0 cooldown=0.1 aim=heading muzzle=1.2 spread=8\n";
    let text = BASE.replace("weapon rifle mode=projectile shot=bullet input=fire reload_input=reload owner=tag:hero ammo=3 reload=1 cooldown=0.25 aim=heading muzzle=1.2\n", spread);
    let play = || {
        let mut w = World::new(&GameDef::parse(&text).unwrap(), 5);
        for _ in 0..40 {
            step(&mut w, &["fire"]);
        }
        w.entities().iter().filter(|e| e.kind == "bullet").map(|e| e.velocity).collect::<Vec<_>>()
    };
    let (a, b) = (play(), play());
    assert_eq!(a, b, "same seed, same scatter");
    assert!(a.len() >= 4);
    assert!(a.iter().any(|v| v[0].abs() > 0.5), "and the shots do not all go straight: {a:?}");
    assert!(a.iter().all(|v| (v.iter().map(|c| c * c).sum::<f64>().sqrt() - 20.0).abs() < 1e-6), "at the shot's speed");
}

#[test]
fn aiming_by_the_camera_sends_the_shot_through_the_crosshair() {
    let text = BASE.replace("aim=heading", "aim=camera");
    let mut w = World::new(&GameDef::parse(&format!("{text}spawn dummy at=3,0,10\n")).unwrap(), 5);
    // the camera stands behind and above the hero looking along +z, slightly toward +x, and the crosshair ray meets the dummy
    let mut inputs = Inputs { pressed: vec!["fire".into()], ..Inputs::default() };
    let (cx, cy, cz) = (0.0_f64, 3.0_f64, -5.0_f64);
    let target = [3.0_f64, 0.9, 10.0];
    let (dx, dy, dz) = (target[0] - cx, target[1] - cy, target[2] - cz);
    inputs.axes.insert("camera.x".into(), cx);
    inputs.axes.insert("camera.y".into(), cy);
    inputs.axes.insert("camera.z".into(), cz);
    inputs.axes.insert("camera.yaw".into(), dx.atan2(dz));
    inputs.axes.insert("camera.pitch".into(), dy.atan2(dx.hypot(dz)));
    w.step(&inputs).unwrap();
    run(&mut w, 90);
    assert_eq!(prop(&w, "dummy", "health"), Some(20.0), "the shot went to where the camera was looking, not where the hero faces");
}

#[test]
fn an_enemy_weapon_aims_at_its_target_and_fires_from_a_rule() {
    let text = format!(
        "{BASE}weapon spit mode=projectile shot=bullet input=none owner=kind:dummy ammo=0 cooldown=0.5 aim=tag:hero muzzle=1.0\nspawn dummy at=6,0,6\nrule on=start do=fire:spit\n"
    );
    let mut w = World::new(&GameDef::parse(&text).unwrap(), 5);
    step(&mut w, &[]);
    run(&mut w, 120);
    assert!(prop(&w, "hero", "health").unwrap() < 100.0, "the dummy shot the hero: {:?}", prop(&w, "hero", "health"));
}

#[test]
fn ammo_and_cooldown_survive_a_save() {
    let mut w = world("");
    step(&mut w, &["fire"]);
    run(&mut w, 5);
    let saved = w.snapshot().to_json();
    let mut other = world("");
    other.restore(&ve_game::Snapshot::from_json(&saved).unwrap()).unwrap();
    assert_eq!(other.hash(), w.hash());
    step(&mut w, &["fire"]);
    step(&mut other, &["fire"]);
    assert_eq!(other.hash(), w.hash(), "and they go on identically (the cooldown was still running)");
    assert_eq!(count(&w, "bullet"), 1);
}

#[test]
fn the_combat_declarations_survive_a_text_round_trip_and_mistakes_are_refused() {
    let def = GameDef::parse(BASE).unwrap();
    let again = GameDef::parse(&def.to_text()).unwrap();
    assert_eq!(def.to_text(), again.to_text());
    let refused = |extra: &str| {
        GameDef::parse(&format!("{BASE}{extra}\n")).map_or_else(|errors| errors.into_iter().map(|e| e.message).collect::<Vec<_>>().join(" | "), |_| String::new())
    };
    assert!(refused("health ghost hp=10").contains("ghost"));
    assert!(refused("health hero hp=0").contains("hp"));
    assert!(refused("projectile ghost speed=5").contains("ghost"));
    assert!(refused("projectile bullet speed=-1").contains("speed"));
    assert!(refused("weapon w mode=projectile shot=ghost input=fire owner=tag:hero").contains("ghost"));
    assert!(refused("weapon w mode=hitscan input=fire owner=tag:hero").contains("range"));
    assert!(refused("weapon w mode=laser input=fire owner=tag:hero").contains("mode"));
    assert!(refused("weapon w mode=hitscan range=5 input=fire owner=tag:nobody").contains("nobody"));
    assert!(refused("weapon rifle mode=hitscan range=5 input=fire owner=tag:hero").contains("duplicate"));
    assert!(refused("rule on=start do=fire:ghost").contains("ghost"));
    assert!(refused("rule on=start do=hurt:tag:hero").contains("amount"));
    assert!(refused("rule on=start do=hurt:tag:hero:lots").contains("amount"));
    assert!(refused("rule on=start do=heal:tag:hero:5").is_empty());
}
