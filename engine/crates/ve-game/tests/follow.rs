//! The player as the hero (ADR 0062 phase 3): an input can make the entity with a tag follow the viewer's position, and kinds name the
//! scene part that draws them. Written before the bodies; do not weaken.
use ve_game::{GameDef, Inputs, World};

const GAME: &str = "game name=walk
var score=0
kind player size=0.5 tags=hero part=body
kind coin size=0.4 tags=pickup part=coin_mesh
spawn player at=0,0,0
spawn coin at=5,0,0
input body follow=viewer target=tag:hero
rule on=touch a=tag:hero b=tag:pickup do=add:score:1;destroy:b
goal win when=score>=1
";

fn at(x: f64, y: f64, z: f64) -> Inputs {
    Inputs {
        viewer: Some([x, y, z]),
        ..Inputs::default()
    }
}

#[expect(clippy::unwrap_used, reason = "the fixture always spawns one player")]
fn hero(world: &World) -> [f64; 3] {
    world
        .entities()
        .iter()
        .find(|e| e.kind == "player")
        .map(|e| e.position)
        .unwrap()
}

#[test]
fn the_hero_follows_the_viewer_and_touches_what_the_viewer_walks_into() {
    let def = GameDef::parse(GAME).unwrap();
    let mut world = World::new(&def, 1);
    world.step(&at(2.0, 0.0, 0.0)).unwrap();
    assert_eq!(
        hero(&world).map(f64::to_bits),
        [2.0, 0.0, 0.0].map(f64::to_bits),
        "the hero stands where the viewer stands"
    );
    assert_eq!(
        world.vars()["score"].to_bits(),
        0.0_f64.to_bits(),
        "two metres from the coin touches nothing"
    );
    let outcome = world.step(&at(5.0, 0.0, 0.0)).unwrap();
    assert_eq!(
        world.vars()["score"].to_bits(),
        1.0_f64.to_bits(),
        "walking into the coin picks it up"
    );
    assert_eq!(outcome.status, ve_game::Status::Won);
}

#[test]
fn without_a_viewer_the_hero_stays_put() {
    let def = GameDef::parse(GAME).unwrap();
    let mut world = World::new(&def, 1);
    world.step(&Inputs::default()).unwrap();
    assert_eq!(
        hero(&world).map(f64::to_bits),
        [0.0, 0.0, 0.0].map(f64::to_bits)
    );
}

#[test]
fn kinds_name_the_part_that_draws_them_and_the_text_keeps_it() {
    let def = GameDef::parse(GAME).unwrap();
    let parts: Vec<_> = def
        .kinds
        .iter()
        .map(|k| (k.name.as_str(), k.part.as_deref()))
        .collect();
    assert_eq!(
        parts,
        vec![("player", Some("body")), ("coin", Some("coin_mesh"))]
    );
    let again = GameDef::parse(&def.to_text()).unwrap();
    assert_eq!(def, again, "follow and part survive printing");
    assert!(def.to_text().contains("follow=viewer") && def.to_text().contains("part=coin_mesh"));
}

#[test]
fn a_follow_input_that_names_an_unknown_source_is_refused_with_a_located_error() {
    let text = GAME.replace("follow=viewer", "follow=moon");
    let errors = GameDef::parse(&text).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| e.line == 7 && e.message.contains("follow")),
        "{errors:?}"
    );
}

#[test]
fn the_viewer_starts_where_the_followed_hero_was_spawned() {
    let text = GAME.replace("spawn player at=0,0,0", "spawn player at=7,2,-3");
    let world = World::new(&GameDef::parse(&text).unwrap(), 1);
    assert_eq!(
        world.viewer_start(),
        Some([7.0, 2.0, -3.0]),
        "so a runtime can put the player there when the game loads"
    );
    let without = GAME.replace("input body follow=viewer target=tag:hero\n", "");
    assert_eq!(
        World::new(&GameDef::parse(&without).unwrap(), 1).viewer_start(),
        None,
        "no follow input, no start"
    );
}

#[test]
fn every_declared_kind_is_listed_even_when_none_of_it_is_alive() {
    let mut world = World::new(&GameDef::parse(GAME).unwrap(), 1);
    world.step(&at(5.0, 0.0, 0.0)).unwrap();
    assert_eq!(
        world.entities().iter().filter(|e| e.kind == "coin").count(),
        0,
        "the coin was picked up"
    );
    let parts: Vec<_> = world
        .kinds()
        .iter()
        .filter_map(|k| k.part.as_deref())
        .collect();
    assert_eq!(
        parts,
        vec!["body", "coin_mesh"],
        "a renderer still knows which parts the game binds"
    );
}

#[test]
fn snapshots_restore_a_followed_hero() {
    let def = GameDef::parse(GAME).unwrap();
    let mut world = World::new(&def, 1);
    world.step(&at(1.0, 0.0, 1.0)).unwrap();
    let saved = world.snapshot();
    world.step(&at(3.0, 0.0, 3.0)).unwrap();
    world.restore(&saved).unwrap();
    assert_eq!(
        hero(&world).map(f64::to_bits),
        [1.0, 0.0, 1.0].map(f64::to_bits)
    );
}
