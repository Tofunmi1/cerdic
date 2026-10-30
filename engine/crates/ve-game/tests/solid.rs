//! Solid kinds: a kind may be a box the hero cannot enter (a door, a wall, a gate), and a bot can pursue targets in order. Without these a "locked" door is only
//! a thing that disappears. Written before the bodies; do not weaken.
#![expect(
    clippy::unwrap_used,
    clippy::float_cmp,
    reason = "fixtures unwrap definitions that are valid by construction and compare variables that only ever hold whole numbers"
)]
use ve_game::playtest::{Policy, run};
use ve_game::{GameDef, Inputs, Status, World};

const VAULT: &str = "game name=vault
var has_key=0
kind player size=0.6 tags=hero
kind key size=0.8 tags=key
kind door size=2 tags=gate solid=0.5,2.6,7.6
kind exit size=0.8 tags=exit
spawn player at=-12,0,0
spawn key at=-4,0.5,2
spawn door at=4,1.3,0
spawn exit at=12,0.05,0
input walk follow=viewer target=tag:hero
rule on=touch a=tag:hero b=tag:key do=add:has_key:1;destroy:b
rule on=touch a=tag:hero b=tag:gate when=has_key>=1 do=destroy:b
rule on=touch a=tag:hero b=tag:exit do=win
";

fn at(x: f64, y: f64, z: f64) -> Inputs {
    Inputs {
        viewer: Some([x, y, z]),
        ..Inputs::default()
    }
}

fn hero_x(world: &World) -> f64 {
    world
        .entities()
        .iter()
        .find(|e| e.kind == "player")
        .map(|e| e.position[0])
        .unwrap()
}

#[test]
fn the_solid_extents_parse_print_and_round_trip() {
    let def = GameDef::parse(VAULT).unwrap();
    let door = def.kinds.iter().find(|k| k.name == "door").unwrap();
    assert_eq!(door.solid, Some([0.5, 2.6, 7.6]));
    assert!(
        def.kinds
            .iter()
            .find(|k| k.name == "key")
            .unwrap()
            .solid
            .is_none()
    );
    let again = GameDef::parse(&def.to_text()).unwrap();
    assert_eq!(def, again);
    assert!(def.to_text().contains("solid=0.5,2.6,7.6"));
}

#[test]
fn bad_solid_extents_are_a_located_error() {
    for bad in ["solid=1,2", "solid=a,b,c", "solid=0,1,1", "solid=1,1,-2"] {
        let text = VAULT.replace("solid=0.5,2.6,7.6", bad);
        let errors = GameDef::parse(&text).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.line == 5 && e.message.contains("solid")),
            "{bad}: {errors:?}"
        );
    }
}

#[test]
fn a_closed_solid_door_stops_the_hero_that_follows_the_viewer() {
    let def = GameDef::parse(VAULT).unwrap();
    let mut world = World::new(&def, 1);
    // The player walks from the spawn straight toward the exit, along the corridor, through where the door stands.
    let mut x = -12.0;
    while x < 12.0 {
        x += 0.05;
        world.step(&at(x, 0.0, 0.0)).unwrap();
        assert!(
            hero_x(&world) < 4.0 - 0.25,
            "the hero never gets into the door: hero {} at viewer {x}",
            hero_x(&world)
        );
    }
    assert!(
        hero_x(&world) > 2.9,
        "but it did get up to it: {}",
        hero_x(&world)
    );
    assert_ne!(world.status(), Status::Won, "so the exit was not reached");
}

#[test]
fn the_hero_slides_along_the_door_not_through_it() {
    let def = GameDef::parse(VAULT).unwrap();
    let mut world = World::new(&def, 1);
    world.step(&at(3.0, 0.0, 3.0)).unwrap();
    // Aimed into the door from the side: the hero is held out of the box on the axis it is nearest to leaving by.
    world.step(&at(4.1, 0.0, 3.0)).unwrap();
    let hero = world
        .entities()
        .iter()
        .find(|e| e.kind == "player")
        .unwrap()
        .position;
    let inside = (hero[0] - 4.0).abs() < 0.25 && (hero[2]).abs() < 3.8 && hero[1] < 2.6;
    assert!(!inside, "the hero is not inside the door box: {hero:?}");
}

#[test]
fn once_the_door_is_gone_nothing_stops_the_hero() {
    let def = GameDef::parse(VAULT).unwrap();
    let mut world = World::new(&def, 1);
    world.step(&at(-4.0, 0.5, 2.0)).unwrap();
    assert_eq!(world.vars()["has_key"], 1.0, "took the key");
    world.step(&at(4.0, 0.0, 0.0)).unwrap();
    assert!(
        !world.entities().iter().any(|e| e.kind == "door"),
        "the door opened"
    );
    world.step(&at(8.0, 0.0, 0.0)).unwrap();
    assert!(
        (hero_x(&world) - 8.0).abs() < 1e-9,
        "and the hero walked through"
    );
}

#[test]
fn a_hero_held_in_front_of_the_door_is_free_to_walk_away_even_if_the_viewer_jumped_far() {
    let def = GameDef::parse(VAULT).unwrap();
    let mut world = World::new(&def, 1);
    world.step(&at(4.0, 0.0, 0.0)).unwrap();
    world.step(&at(12.0, 0.0, 0.0)).unwrap();
    assert!(hero_x(&world) < 4.0 - 0.25, "held by the shut door");
    world.step(&at(-4.0, 0.5, 2.0)).unwrap();
    assert_eq!(
        world.vars()["has_key"],
        1.0,
        "the viewer went back for the key and the hero came with it: {}",
        hero_x(&world)
    );
}

#[test]
fn a_viewer_that_jumps_across_a_shut_door_does_not_put_the_hero_behind_it() {
    let def = GameDef::parse(VAULT).unwrap();
    for jump in [8.0, 12.0, 40.0] {
        let mut world = World::new(&def, 1);
        world.step(&at(-12.0, 0.0, 0.0)).unwrap();
        world.step(&at(jump, 0.0, 0.0)).unwrap();
        assert!(
            (hero_x(&world) - 3.15).abs() < 1e-6,
            "from the near side to {jump}: the hero is held in front of the door, at {}",
            hero_x(&world)
        );
    }
    // From the far side the hero is held on the far side.
    let behind =
        GameDef::parse(&VAULT.replace("spawn player at=-12,0,0", "spawn player at=20,0,0"))
            .unwrap();
    let mut world = World::new(&behind, 1);
    world.step(&at(-12.0, 0.0, 0.0)).unwrap();
    assert!(
        (hero_x(&world) - 4.85).abs() < 1e-6,
        "a hero that started behind the door stays behind it: {}",
        hero_x(&world)
    );
}

#[test]
fn a_kind_without_solid_is_not_an_obstacle() {
    let text = VAULT.replace(" solid=0.5,2.6,7.6", "");
    let mut world = World::new(&GameDef::parse(&text).unwrap(), 1);
    world.step(&at(4.0, 0.0, 0.0)).unwrap();
    assert!((hero_x(&world) - 4.0).abs() < 1e-9);
}

#[test]
fn a_bot_that_goes_straight_for_the_exit_is_stopped_by_the_locked_door() {
    let def = GameDef::parse(VAULT).unwrap();
    let report = run(
        &def,
        1,
        &Policy::Seek {
            hero: "hero".to_owned(),
            target: "exit".to_owned(),
        },
        3_600,
    );
    assert_ne!(report.status, Status::Won, "no key, no exit: {report:?}");
}

#[test]
fn targets_can_be_listed_in_order_and_the_bot_finishes_the_puzzle() {
    let def = GameDef::parse(VAULT).unwrap();
    let report = run(
        &def,
        1,
        &Policy::Seek {
            hero: "hero".to_owned(),
            target: "key,gate,exit".to_owned(),
        },
        3_600,
    );
    assert_eq!(
        report.status,
        Status::Won,
        "key, then the door, then the exit: {report:?}"
    );
    assert_eq!(
        report.never_fired,
        Vec::<usize>::new(),
        "every rule did its part"
    );
    // 25 m of corridor plus the detour for the key, at a walking pace: well under two minutes.
    assert!(report.steps < 3_000, "{} steps", report.steps);
}
