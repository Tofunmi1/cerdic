//! Which way an entity faces (ADR 0066): a game entity bound to an animated model must turn to where it walks. `Entity::heading` is the yaw in
//! radians about +Y with 0 facing +Z (atan2(dx, dz) of the last step it moved), unchanged while it stands still, and a spawn may give the
//! first one (`heading=DEGREES`). It is derived data: it is not part of the state hash. Written before the body; do not weaken.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use std::f64::consts::{FRAC_PI_2, PI};

use ve_game::{GameDef, Inputs, World};

const GAME: &str = "game name=walk\nkind player size=0.5 tags=hero part=body\nspawn player at=0,0,0\ninput body follow=viewer target=tag:hero\n";

fn at(x: f64, z: f64) -> Inputs {
    Inputs {
        viewer: Some([x, 0.0, z]),
        ..Inputs::default()
    }
}

fn heading(world: &World) -> f64 {
    world.entities()[0].heading
}

#[test]
fn an_entity_that_has_not_moved_faces_plus_z() {
    let world = World::new(&GameDef::parse(GAME).unwrap(), 1);
    assert_eq!(heading(&world).to_bits(), 0.0_f64.to_bits());
}

#[test]
fn it_turns_to_face_the_way_it_moves() {
    let mut world = World::new(&GameDef::parse(GAME).unwrap(), 1);
    world.step(&at(3.0, 0.0)).unwrap();
    assert!(
        (heading(&world) - FRAC_PI_2).abs() < 1e-9,
        "walking +X faces +X: {}",
        heading(&world)
    );
    world.step(&at(3.0, -2.0)).unwrap();
    assert!(
        (heading(&world).abs() - PI).abs() < 1e-9,
        "walking -Z faces -Z: {}",
        heading(&world)
    );
    world.step(&at(-1.0, -2.0)).unwrap();
    assert!(
        (heading(&world) + FRAC_PI_2).abs() < 1e-9,
        "walking -X faces -X: {}",
        heading(&world)
    );
}

#[test]
fn it_keeps_its_heading_while_it_stands_still() {
    let mut world = World::new(&GameDef::parse(GAME).unwrap(), 1);
    world.step(&at(3.0, 0.0)).unwrap();
    let turned = heading(&world);
    for _ in 0..5 {
        world.step(&at(3.0, 0.0)).unwrap();
    }
    assert_eq!(heading(&world).to_bits(), turned.to_bits());
}

#[test]
fn a_spawn_can_face_a_direction_to_begin_with() {
    let text = "game name=a\nkind guard size=0.5 part=guard_body\nspawn guard at=0,0,0 heading=90\nspawn guard at=2,0,0\n";
    let world = World::new(&GameDef::parse(text).unwrap(), 1);
    assert!(
        (world.entities()[0].heading - FRAC_PI_2).abs() < 1e-9,
        "{}",
        world.entities()[0].heading
    );
    assert_eq!(world.entities()[1].heading.to_bits(), 0.0_f64.to_bits());
}

#[test]
fn a_bad_heading_is_refused_with_the_line() {
    let diagnostics =
        GameDef::parse("game name=a\nkind guard size=0.5\nspawn guard at=0,0,0 heading=left\n")
            .unwrap_err();
    assert!(
        diagnostics.iter().any(|d| d.message.contains("heading")),
        "{diagnostics:?}"
    );
}

#[test]
fn heading_is_not_part_of_the_state_hash() {
    // The same game stepped to the same positions hashes the same whichever way the entities were facing on the way there.
    let text = "game name=a\nkind guard size=0.5\nspawn guard at=0,0,0\n";
    let faced = "game name=a\nkind guard size=0.5\nspawn guard at=0,0,0 heading=45\n";
    let a = World::new(&GameDef::parse(text).unwrap(), 1);
    let b = World::new(&GameDef::parse(faced).unwrap(), 1);
    assert_eq!(a.hash(), b.hash());
}
