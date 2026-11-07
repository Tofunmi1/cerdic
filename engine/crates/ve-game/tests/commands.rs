//! Commands a host gives the game from outside its rules (a script placing the pieces of a turn-based game): spawn an entity of a declared
//! kind, put an entity somewhere (and optionally facing somewhere), change one of its numbers, remove it. Each is checked and refused with a
//! reason that names the fix. Written before the body; do not weaken.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use std::f64::consts::FRAC_PI_2;

use ve_game::{GameDef, Inputs, World};

const GAME: &str =
    "game name=tactics\nkind unit size=0.4 tags=unit part=body\nkind prop size=0.2 part=body\n";

fn world() -> World {
    World::new(&GameDef::parse(GAME).unwrap(), 1)
}

#[test]
fn a_spawned_entity_has_the_kind_the_place_and_the_heading() {
    let mut world = world();
    let id = world.command_spawn("unit", [3.0, 0.0, 4.0], 0.5).unwrap();
    let entity = world
        .entities()
        .iter()
        .find(|entity| entity.id == id)
        .unwrap();
    assert_eq!(entity.kind, "unit");
    assert_eq!(entity.position, [3.0, 0.0, 4.0]);
    assert_eq!(entity.heading.to_bits(), 0.5_f64.to_bits());
    assert!(entity.props.is_empty());
}

#[test]
fn ids_are_never_reused() {
    let mut world = world();
    let first = world.command_spawn("unit", [0.0; 3], 0.0).unwrap();
    world.command_remove(first).unwrap();
    let second = world.command_spawn("unit", [0.0; 3], 0.0).unwrap();
    assert_ne!(first, second);
}

#[test]
fn an_unknown_kind_is_refused_naming_the_kinds_that_exist() {
    let mut world = world();
    let message = world
        .command_spawn("dragon", [0.0; 3], 0.0)
        .unwrap_err()
        .to_string();
    assert!(
        message.contains("dragon") && message.contains("unit") && message.contains("prop"),
        "{message}"
    );
}

#[test]
fn place_moves_the_entity_and_keeps_the_heading_unless_given_one() {
    let mut world = world();
    let id = world.command_spawn("unit", [0.0; 3], 1.0).unwrap();
    world.command_place(id, [5.0, 0.0, 0.0], None).unwrap();
    assert_eq!(world.entities()[0].position, [5.0, 0.0, 0.0]);
    assert_eq!(world.entities()[0].heading.to_bits(), 1.0_f64.to_bits());
    world
        .command_place(id, [5.0, 0.0, 0.0], Some(FRAC_PI_2))
        .unwrap();
    assert_eq!(world.entities()[0].heading.to_bits(), FRAC_PI_2.to_bits());
}

#[test]
fn a_placed_entity_stays_where_it_was_put_when_the_game_steps() {
    let mut world = world();
    let id = world.command_spawn("unit", [0.0; 3], 0.0).unwrap();
    world.command_place(id, [2.0, 0.0, 2.0], None).unwrap();
    for _ in 0..30 {
        world.step(&Inputs::default()).unwrap();
    }
    assert_eq!(world.entities()[0].position, [2.0, 0.0, 2.0]);
}

#[test]
fn an_entity_that_is_not_there_is_refused_by_id() {
    let mut world = world();
    assert!(
        world
            .command_place(99, [0.0; 3], None)
            .unwrap_err()
            .to_string()
            .contains("99")
    );
    assert!(
        world
            .command_remove(99)
            .unwrap_err()
            .to_string()
            .contains("99")
    );
    assert!(
        world
            .command_set_prop(99, "hp", 1.0)
            .unwrap_err()
            .to_string()
            .contains("99")
    );
}

#[test]
fn a_number_can_be_set_and_a_removed_entity_is_gone() {
    let mut world = world();
    let id = world.command_spawn("unit", [0.0; 3], 0.0).unwrap();
    world.command_set_prop(id, "hp", 4.0).unwrap();
    assert_eq!(world.entities()[0].props.get("hp"), Some(&4.0));
    world.command_remove(id).unwrap();
    assert!(world.entities().is_empty());
}

#[test]
fn a_position_that_is_not_a_number_is_refused() {
    let mut world = world();
    let id = world.command_spawn("unit", [0.0; 3], 0.0).unwrap();
    assert!(world.command_place(id, [f64::NAN, 0.0, 0.0], None).is_err());
    assert!(
        world
            .command_spawn("unit", [0.0, f64::INFINITY, 0.0], 0.0)
            .is_err()
    );
}
