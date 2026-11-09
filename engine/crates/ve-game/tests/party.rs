//! Party combat (a GreedFall-style encounter): `brain` gives a kind its own movement and fighting (follow a leader, engage the nearest foe, use a
//! weapon), melee weapons hit an arc, a weapon's `foes` keeps allies from hurting each other, `targeting` picks and remembers the foe the leader
//! faces, `ability` has cooldowns and costs and runs actions, and `dash` and `iframes` make a dodge. Pure and deterministic. Written first.
#![expect(clippy::unwrap_used, clippy::float_cmp, reason = "fixtures are known to be valid and compare exact counts")]
use ve_game::{GameDef, Inputs, World};

const BASE: &str = "game name=party
var kills=0
var refused=0
var used=0
kind hero size=0.4 tags=hero,party
kind pal size=0.4 tags=party
kind imp size=0.5 tags=enemy
kind rock size=1 solid=2,2,2
kind bolt size=0.1 tags=shot
health hero hp=100
health pal hp=80
health imp hp=40 height=1.2
controller hero mode=walk speed=6 accel=200 decel=200
input move axis=x,z target=tag:hero
input power key=press
input dodge key=press
input next key=press
weapon sword mode=melee owner=kind:pal foes=tag:enemy range=2 arc=120 damage=10 cooldown=0.5 aim=tag:enemy muzzle=1
weapon claw mode=melee owner=kind:imp foes=tag:party range=1.8 arc=120 damage=6 cooldown=0.8 aim=tag:party muzzle=1
brain pal foes=tag:enemy leader=tag:hero speed=4 follow=3 engage=14 weapon=sword
brain imp foes=tag:party speed=3 engage=10 weapon=claw
spawn hero at=0,0,0
rule on=killed a=tag:enemy do=add:kills:1
";

fn world(extra: &str) -> World {
    World::new(&GameDef::parse(&format!("{BASE}{extra}")).unwrap(), 9)
}

fn step(world: &mut World, move_x: f64, move_z: f64, pressed: &[&str]) {
    let mut inputs = Inputs { pressed: pressed.iter().map(|p| (*p).to_owned()).collect(), ..Inputs::default() };
    inputs.axes.insert("move.x".into(), move_x);
    inputs.axes.insert("move.z".into(), move_z);
    world.step(&inputs).unwrap();
}

fn run(world: &mut World, steps: usize) {
    for _ in 0..steps {
        step(world, 0.0, 0.0, &[]);
    }
}

fn at(world: &World, kind: &str) -> [f64; 3] {
    world.entities().iter().find(|e| e.kind == kind).unwrap().position
}

fn health(world: &World, kind: &str) -> Option<f64> {
    world.entities().iter().find(|e| e.kind == kind).and_then(|e| e.props.get("health").copied())
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] - b[0]).hypot(a[2] - b[2])
}

#[test]
fn a_companion_follows_its_leader_and_keeps_its_distance() {
    let mut w = world("spawn pal at=-2,0,-2\n");
    for _ in 0..120 {
        step(&mut w, 1.0, 0.0, &[]);
    }
    assert!(at(&w, "hero")[0] > 10.0, "the hero ran off: {:?}", at(&w, "hero"));
    // the companion is slower than the hero (4 against 6 m/s) but catches up while the hero stands
    run(&mut w, 60 * 5);
    let (hero, pal) = (at(&w, "hero"), at(&w, "pal"));
    let gap = distance(hero, pal);
    assert!((2.0..=4.0).contains(&gap), "the companion stops at about its follow distance of 3: {gap}");
    let stopped = pal;
    run(&mut w, 90);
    assert!(distance(at(&w, "pal"), stopped) < 0.3, "and stands still once it has caught up");
}

#[test]
fn a_companion_engages_the_nearest_foe_and_kills_it() {
    let mut w = world("spawn pal at=-2,0,0\nspawn imp at=7,0,0\n");
    run(&mut w, 60 * 8);
    assert_eq!(w.vars().get("kills").copied(), Some(1.0), "the foe is dead");
    assert!(health(&w, "pal").unwrap() > 0.0, "and the companion lived");
}

#[test]
fn foes_fight_back_against_the_party_but_not_each_other() {
    let mut w = world("spawn pal at=-1,0,0\nspawn imp at=3,0,0\nspawn imp at=3.5,0,0.5\n");
    run(&mut w, 60 * 6);
    let hurt = health(&w, "pal").unwrap() < 80.0 || health(&w, "hero").unwrap() < 100.0 || w.entities().iter().all(|e| e.kind != "pal");
    assert!(hurt, "the imps attacked the party");
    assert!(w.entities().iter().filter(|e| e.kind == "imp").all(|e| e.props["health"] > 0.0) || w.vars()["kills"] > 0.0, "imps never hurt imps: any that died were killed by the party");
}

#[test]
fn a_weapons_foes_keep_its_shots_off_its_friends() {
    // the companion shoots at a foe straight past the hero, who stands in the line of fire
    let text = "projectile bolt speed=20 life=2 damage=30\nweapon bow mode=projectile shot=bolt owner=kind:pal foes=tag:enemy ammo=0 cooldown=0.3 aim=tag:enemy muzzle=1\nbrain pal foes=tag:enemy leader=tag:hero speed=4 follow=3 engage=20 weapon=bow keep=8\nspawn pal at=-6,0,0\nspawn imp at=8,0,0\n";
    let base = BASE.replace("brain pal foes=tag:enemy leader=tag:hero speed=4 follow=3 engage=14 weapon=sword\n", "");
    let mut w = World::new(&GameDef::parse(&format!("{base}{text}")).unwrap(), 9);
    run(&mut w, 60 * 3);
    assert_eq!(health(&w, "hero"), Some(100.0), "the hero in the way was not shot");
}

#[test]
fn a_brain_does_not_walk_through_a_wall() {
    let mut w = world("spawn pal at=-6,0,0\nspawn rock at=-3,1,0\n");
    for _ in 0..240 {
        step(&mut w, 0.0, 0.0, &[]);
    }
    let pal = at(&w, "pal");
    assert!(pal[0] < -4.0 || pal[2].abs() > 1.3, "the companion goes around or stops at the rock (spans x -4..-2, z -1..1): {pal:?}");
}

#[test]
fn the_same_inputs_give_the_same_battle() {
    let play = || {
        let mut w = world("spawn pal at=-2,0,0\nspawn imp at=7,0,0\nspawn imp at=8,0,2\n");
        run(&mut w, 60 * 6);
        w.hash()
    };
    assert_eq!(play(), play());
}

const TARGETING: &str = "targeting leader=tag:hero foes=tag:enemy range=25 cone=80\n";

#[test]
fn targeting_picks_the_foe_the_leader_faces_and_keeps_it_until_it_is_gone() {
    let mut w = world(&format!("{TARGETING}spawn imp at=0,0,10\nspawn imp at=10,0,0\n"));
    // the hero faces +z at first: the foe straight ahead is chosen
    run(&mut w, 2);
    let imps: Vec<_> = w.entities().iter().filter(|e| e.kind == "imp").collect();
    assert_eq!(imps[0].props.get("targeted").copied(), Some(1.0), "the one ahead");
    assert_eq!(imps[1].props.get("targeted").copied().unwrap_or(0.0), 0.0);
    // turning toward the other does not steal the target (it is sticky) ...
    for _ in 0..40 {
        step(&mut w, -1.0, 0.0, &[]);
    }
    assert_eq!(w.entities().iter().filter(|e| e.kind == "imp" && e.props.get("targeted") == Some(&1.0)).count(), 1, "exactly one target at a time");
}

#[test]
fn next_cycles_the_target_to_the_next_nearest_foe() {
    let mut w = world(&format!("{TARGETING}rule on=input:next do=target:next\nspawn imp at=0,0,6\nspawn imp at=3,0,9\nspawn imp at=-3,0,12\n"));
    run(&mut w, 2);
    let target = |w: &World| w.entities().iter().filter(|e| e.kind == "imp").position(|e| e.props.get("targeted") == Some(&1.0));
    assert_eq!(target(&w), Some(0), "the nearest at first");
    step(&mut w, 0.0, 0.0, &["next"]);
    assert_eq!(target(&w), Some(1), "then the next one out");
    step(&mut w, 0.0, 0.0, &["next"]);
    assert_eq!(target(&w), Some(2));
    step(&mut w, 0.0, 0.0, &["next"]);
    assert_eq!(target(&w), Some(0), "and round again");
}

#[test]
fn a_dead_target_hands_over_to_the_next_foe() {
    let mut w = world(&format!("{TARGETING}input kill key=press\nrule on=input:kill do=hurt:tag:enemy:1000\nspawn imp at=0,0,6\n"));
    run(&mut w, 2);
    assert!(w.entities().iter().any(|e| e.props.get("targeted") == Some(&1.0)));
    step(&mut w, 0.0, 0.0, &["kill"]);
    run(&mut w, 2);
    assert!(w.entities().iter().all(|e| e.props.get("targeted").copied().unwrap_or(0.0) == 0.0), "nothing to target");
}

/// Pressing `give` puts thirty focus on the hero (the tests have no other way to fill a prop).
const GIVE: &str = "input give key=press\nrule on=input:give do=setprop:tag:hero:focus:30\n";

const ABILITY: &str ="var focus=0\nability smash owner=tag:hero input=power cooldown=2 cost=10 cost_prop=focus range=6 needs_target=1 do=hurt:b:25\nrule on=refused:smash do=add:refused:1\nrule on=ability:smash do=add:used:1\n";

#[test]
fn an_ability_runs_its_actions_on_the_target_then_waits_out_its_cooldown() {
    let mut w = world(&format!("{TARGETING}{ABILITY}{GIVE}spawn imp at=0,0,4\n"));
    run(&mut w, 2);
    let hero_focus = |w: &World| w.entities().iter().find(|e| e.kind == "hero").unwrap().props.get("focus").copied();
    step(&mut w, 0.0, 0.0, &["give"]);
    step(&mut w, 0.0, 0.0, &["power"]);
    assert_eq!(health(&w, "imp"), Some(15.0), "25 damage to the target");
    assert_eq!(w.vars()["used"], 1.0);
    assert_eq!(hero_focus(&w), Some(20.0), "ten focus spent");
    let cooling = w.entities().iter().find(|e| e.kind == "hero").unwrap().props["cd_smash"];
    assert!(cooling > 1.9 && cooling <= 2.0, "two seconds to wait: {cooling}");
    step(&mut w, 0.0, 0.0, &["power"]);
    assert_eq!(w.vars()["refused"], 1.0, "pressing again while it cools down is refused");
    assert_eq!(health(&w, "imp"), Some(15.0));
    run(&mut w, 125);
    step(&mut w, 0.0, 0.0, &["power"]);
    assert_eq!(health(&w, "imp"), None, "after the cooldown it works again (and finishes the imp)");
}

#[test]
fn an_ability_is_refused_without_the_focus_or_without_a_target_in_range() {
    let mut poor = world(&format!("{TARGETING}{ABILITY}{GIVE}spawn imp at=0,0,4\n"));
    run(&mut poor, 2);
    step(&mut poor, 0.0, 0.0, &["power"]);
    assert_eq!(poor.vars()["refused"], 1.0, "no focus");
    assert_eq!(health(&poor, "imp"), Some(40.0));
    let mut far = world(&format!("{TARGETING}{ABILITY}{GIVE}spawn imp at=0,0,20\n"));
    step(&mut far, 0.0, 0.0, &["give"]);
    run(&mut far, 2);
    step(&mut far, 0.0, 0.0, &["power"]);
    assert_eq!(far.vars()["refused"], 1.0, "the target is beyond range 6");
    let mut none = world(&format!("{TARGETING}{ABILITY}{GIVE}"));
    step(&mut none, 0.0, 0.0, &["give"]);
    step(&mut none, 0.0, 0.0, &["power"]);
    assert_eq!(none.vars()["refused"], 1.0, "no target at all");
}

#[test]
fn a_blast_hurts_every_foe_around_the_user_and_nobody_else() {
    let text = "input boom key=press\nrule on=input:boom do=blast:tag:hero:5:30@tag:enemy\nspawn pal at=-3,0,-3\nspawn imp at=2,0,2\nspawn imp at=-3,0,1\nspawn imp at=9,0,9\n";
    let mut w = world(text);
    step(&mut w, 0.0, 0.0, &["boom"]);
    let healths: Vec<f64> = w.entities().iter().filter(|e| e.kind == "imp").map(|e| e.props["health"]).collect();
    assert_eq!(healths, vec![10.0, 10.0, 40.0], "the two within five metres took 30, the far one nothing");
    assert_eq!(health(&w, "pal"), Some(80.0), "allies are not hurt by the hero's blast");
}

#[test]
fn a_dodge_is_a_quick_dash_with_invulnerability_frames() {
    let text = "health hero hp=100 iframes=0\ninput hit key=press\nrule on=input:hit do=hurt:tag:hero:10\nrule on=input:dodge do=dash:tag:hero:12:0.25;iframes:tag:hero:0.4\n";
    let mut w = world(text);
    step(&mut w, 1.0, 0.0, &["dodge"]);
    for _ in 0..16 {
        step(&mut w, 1.0, 0.0, &[]);
    }
    assert!(at(&w, "hero")[0] > 2.5, "a dash of 12 m/s for a quarter second covers about 3 m: {:?}", at(&w, "hero"));
    let mut dodging = world(text);
    step(&mut dodging, 1.0, 0.0, &["dodge"]);
    step(&mut dodging, 1.0, 0.0, &["hit"]);
    assert_eq!(health(&dodging, "hero"), Some(100.0), "the hit during the dodge did nothing");
    run(&mut dodging, 30);
    step(&mut dodging, 0.0, 0.0, &["hit"]);
    assert_eq!(health(&dodging, "hero"), Some(90.0), "after it, hits land again");
}

#[test]
fn party_declarations_round_trip_and_mistakes_are_refused() {
    let def = GameDef::parse(&format!("{BASE}{TARGETING}{ABILITY}")).unwrap();
    let again = GameDef::parse(&def.to_text()).unwrap();
    assert_eq!(def.to_text(), again.to_text());
    let refused = |extra: &str| GameDef::parse(&format!("{BASE}{extra}\n")).map_or_else(|errors| errors.into_iter().map(|e| e.message).collect::<Vec<_>>().join(" | "), |_| String::new());
    assert!(refused("brain ghost foes=tag:enemy speed=3").contains("ghost"));
    assert!(refused("brain imp foes=tag:enemy speed=0").contains("speed"));
    assert!(refused("brain pal foes=tag:enemy speed=3 weapon=nothing").contains("nothing"));
    assert!(refused("weapon w mode=melee owner=kind:pal range=0 arc=90").contains("range"));
    assert!(refused("weapon w mode=melee owner=kind:pal range=2 arc=400").contains("arc"));
    assert!(refused("targeting leader=tag:nobody foes=tag:enemy").contains("nobody"));
    assert!(refused("ability a owner=tag:hero input=nothing cooldown=1").contains("nothing"));
    assert!(refused("ability a owner=tag:hero input=power cooldown=-1").contains("cooldown"));
    assert!(refused("rule on=start do=dash:tag:hero:12").contains("dash"));
    assert!(refused("rule on=start do=blast:tag:hero:5").contains("blast"));
    assert!(refused("rule on=start do=iframes:tag:hero:0.4").is_empty());
}
