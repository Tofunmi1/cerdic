//! Game flow (F3 of `plans/game-foundations-plan.md`): named states (a `frozen` one stops the simulation: a title, a pause, a game over),
//! rules that move between them (`go:NAME`) and are limited to one (`state=NAME`), `restart`, levels (`level:NAME` swaps the level's
//! spawns and puts the hero at its start), and checkpoints (`checkpoint`, `respawn`). Pure and deterministic. Written before the body.
#![expect(clippy::unwrap_used, clippy::float_cmp, reason = "fixtures are known to be valid and compare exact rest values")]
use ve_game::{GameDef, Inputs, Status, World};

const GAME: &str = "game name=flow
var coins=0
var entered=0
kind hero size=0.4 speed=5 tags=hero
kind coin size=0.3 tags=coin
controller hero mode=walk speed=5 accel=100 decel=100
input move axis=x,z target=tag:hero
input start key=press
input pause key=press
state title frozen=1
state playing
state paused frozen=1
state game_over frozen=1
state win frozen=1
spawn hero at=0,0,0
spawn coin at=12,0,0
rule on=input:start state=title do=go:playing
rule on=input:pause state=playing do=go:paused
rule on=input:pause state=paused do=go:playing
rule on=state:playing do=add:entered:1
rule on=touch a=tag:hero b=tag:coin do=add:coins:1;destroy:b
goal win when=coins>=1
";

fn world() -> World {
    World::new(&GameDef::parse(GAME).unwrap(), 7)
}

fn press(world: &mut World, names: &[&str], move_x: f64) {
    let mut inputs = Inputs::default();
    inputs.axes.insert("move.x".into(), move_x);
    inputs.pressed = names.iter().map(|n| (*n).to_owned()).collect();
    world.step(&inputs).unwrap();
}

fn hero_x(world: &World) -> f64 {
    world.entities().iter().find(|e| e.kind == "hero").unwrap().position[0]
}

#[test]
fn the_game_starts_in_its_first_state_and_a_frozen_state_does_not_advance_the_world() {
    let mut w = world();
    assert_eq!(w.flow_state(), "title");
    let before = w.hash();
    for _ in 0..120 {
        press(&mut w, &[], 1.0);
    }
    assert_eq!(w.hash(), before, "a title screen is purely still, whatever the stick says");
    assert_eq!(hero_x(&w), 0.0);
}

#[test]
fn a_rule_limited_to_a_state_moves_the_game_on_and_the_world_runs() {
    let mut w = world();
    press(&mut w, &["start"], 0.0);
    assert_eq!(w.flow_state(), "playing");
    assert_eq!(w.vars().get("entered").copied(), Some(1.0), "entering a state raises state:NAME");
    for _ in 0..30 {
        press(&mut w, &[], 1.0);
    }
    assert!(hero_x(&w) > 0.5, "now the hero moves: {}", hero_x(&w));
    press(&mut w, &["start"], 0.0);
    assert_eq!(w.flow_state(), "playing", "the start rule is only for the title");
}

#[test]
fn pausing_is_pure_and_resuming_continues() {
    let mut w = world();
    press(&mut w, &["start"], 0.0);
    for _ in 0..20 {
        press(&mut w, &[], 1.0);
    }
    press(&mut w, &["pause"], 1.0);
    assert_eq!(w.flow_state(), "paused");
    let frozen = w.hash();
    for _ in 0..200 {
        press(&mut w, &[], 1.0);
    }
    assert_eq!(w.hash(), frozen, "nothing changes while paused");
    press(&mut w, &["pause"], 0.0);
    assert_eq!(w.flow_state(), "playing");
    let x = hero_x(&w);
    for _ in 0..20 {
        press(&mut w, &[], 1.0);
    }
    assert!(hero_x(&w) > x, "and it runs again");
}

#[test]
fn winning_and_losing_enter_the_states_with_those_names() {
    let mut w = world();
    press(&mut w, &["start"], 0.0);
    for _ in 0..300 {
        press(&mut w, &[], 1.0);
        if w.status() != Status::Playing {
            break;
        }
    }
    assert_eq!(w.status(), Status::Won);
    assert_eq!(w.flow_state(), "win", "the goal put the game in its win state");
    let still = w.hash();
    press(&mut w, &[], 1.0);
    assert_eq!(w.hash(), still, "and a win screen is frozen");
}

#[test]
fn restart_brings_back_the_start_and_enters_the_state_asked_for() {
    let text = format!("{GAME}rule on=input:pause state=win do=restart:playing\nrule on=input:start state=game_over do=restart\n");
    let mut w = World::new(&GameDef::parse(&text).unwrap(), 7);
    press(&mut w, &["start"], 0.0);
    for _ in 0..300 {
        press(&mut w, &[], 1.0);
        if w.status() != Status::Playing {
            break;
        }
    }
    assert_eq!(w.flow_state(), "win");
    press(&mut w, &["pause"], 0.0);
    assert_eq!(w.flow_state(), "playing", "restart:playing");
    assert_eq!(w.status(), Status::Playing);
    assert_eq!(hero_x(&w), 0.0, "the hero is back at its spawn");
    assert_eq!(w.vars().get("coins").copied(), Some(0.0), "variables are back to their starting values");
    assert_eq!(w.entities().iter().filter(|e| e.kind == "coin").count(), 1, "and the coin is back");
}

#[test]
fn a_restarted_game_plays_exactly_like_a_new_one() {
    let text = format!("{GAME}rule on=input:pause state=win do=restart:playing\n");
    let def = GameDef::parse(&text).unwrap();
    let mut a = World::new(&def, 7);
    press(&mut a, &["start"], 0.0);
    for _ in 0..300 {
        press(&mut a, &[], 1.0);
        if a.status() != Status::Playing {
            break;
        }
    }
    press(&mut a, &["pause"], 0.0);
    let mut b = World::new(&def, 7);
    press(&mut b, &["start"], 0.0);
    for _ in 0..40 {
        press(&mut a, &[], 1.0);
        press(&mut b, &[], 1.0);
    }
    // the hash also counts how often each rule fired (b pressed start, a restarted into playing), so compare what a player can see
    assert_eq!(a.entities(), b.entities());
    assert_eq!(a.vars(), b.vars());
    assert_eq!((a.flow_state(), a.tick(), a.status()), (b.flow_state(), b.tick(), b.status()));
}

#[test]
fn a_checkpoint_returns_the_hero_to_where_it_was_stored() {
    let text = format!("{GAME}input mark key=press\ninput back key=press\nrule on=input:mark do=checkpoint\nrule on=input:back do=respawn\n");
    let mut w = World::new(&GameDef::parse(&text).unwrap(), 7);
    press(&mut w, &["start"], 0.0);
    for _ in 0..12 {
        press(&mut w, &[], 1.0);
    }
    let stored = hero_x(&w);
    press(&mut w, &["mark"], 1.0);
    let saved_at = hero_x(&w);
    assert!(saved_at >= stored);
    for _ in 0..30 {
        press(&mut w, &[], -1.0);
    }
    assert!(hero_x(&w) < saved_at);
    press(&mut w, &["back"], 0.0);
    assert!((hero_x(&w) - saved_at).abs() < 0.2, "{} against {saved_at}", hero_x(&w));
    assert_eq!(w.entities().iter().find(|e| e.kind == "hero").unwrap().velocity, [0.0; 3], "at rest");
}

const LEVELS: &str = "game name=lv
var gate=0
kind hero size=0.4 tags=hero
kind rock size=0.5
kind star size=0.3
spawn hero at=0,0,0
level one start=1,0,0
level two start=-5,0,5
spawn rock at=3,0,3 level=one
spawn star at=9,0,9 level=two
spawn star at=8,0,9 level=two
input go key=press
rule on=input:go when=gate==0 do=level:two;set:gate:1
rule on=level:two do=add:gate:10
";

#[test]
fn levels_swap_their_spawns_and_put_the_hero_at_the_start() {
    let mut w = World::new(&GameDef::parse(LEVELS).unwrap(), 3);
    let count = |w: &World, kind: &str| w.entities().iter().filter(|e| e.kind == kind).count();
    assert_eq!((count(&w, "rock"), count(&w, "star")), (1, 0), "the first level is loaded");
    assert_eq!(w.entities().iter().find(|e| e.kind == "hero").unwrap().position, [1.0, 0.0, 0.0], "and the hero stands at its start");
    let mut inputs = Inputs::default();
    inputs.pressed.push("go".into());
    w.step(&inputs).unwrap();
    assert_eq!((count(&w, "rock"), count(&w, "star")), (0, 2), "the second level replaced the first");
    assert_eq!(w.entities().iter().find(|e| e.kind == "hero").unwrap().position, [-5.0, 0.0, 5.0]);
    assert_eq!(w.vars().get("gate").copied(), Some(11.0), "level:two was raised and the rule heard it");
}

#[test]
fn the_flow_survives_a_save_and_a_load() {
    let mut w = world();
    press(&mut w, &["start"], 0.0);
    for _ in 0..30 {
        press(&mut w, &[], 1.0);
    }
    press(&mut w, &["pause"], 0.0);
    let saved = w.snapshot().to_json();
    let mut other = world();
    other.restore(&ve_game::Snapshot::from_json(&saved).unwrap()).unwrap();
    assert_eq!(other.flow_state(), "paused");
    assert_eq!(other.hash(), w.hash());
}

#[test]
fn mistakes_in_the_flow_are_refused_with_a_reason() {
    let refused = |extra: &str| {
        GameDef::parse(&format!("game name=bad\nkind hero size=0.4\nstate a\nstate b frozen=1\n{extra}\n"))
            .map_or_else(|errors| errors.into_iter().map(|e| e.message).collect::<Vec<_>>().join(" | "), |_| String::new())
    };
    assert!(refused("rule on=start do=go:nowhere").contains("nowhere"));
    assert!(refused("rule on=start state=nowhere do=add:x:1").contains("nowhere"));
    assert!(refused("state a").contains("duplicate"));
    assert!(refused("state c frozen=maybe").contains("frozen"));
    assert!(refused("spawn hero at=0,0,0 level=ghost").contains("ghost"));
    assert!(refused("level one\nlevel one").contains("duplicate"));
    assert!(refused("rule on=start do=level:ghost").contains("ghost"));
    assert!(refused("rule on=start do=restart:nowhere").contains("nowhere"));
    assert!(refused("level one start=1,2").contains("start"));
    assert!(refused("rule on=state:nowhere do=add:x:1").contains("state"));
    assert_eq!(refused("rule on=start do=go:b"), "");
}

#[test]
fn games_without_states_are_unchanged() {
    let def = GameDef::parse("game name=old\nkind hero size=0.4 speed=2\nspawn hero at=0,0,0\n").unwrap();
    let mut w = World::new(&def, 1);
    assert_eq!(w.flow_state(), "");
    w.step(&Inputs::default()).unwrap();
    assert_eq!(w.tick(), 1);
}
