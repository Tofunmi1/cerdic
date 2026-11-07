//! A save game: the world's snapshot written as JSON text and read back into a fresh world gives the same state, and the game carries on the same.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_game::{GameDef, Inputs, Snapshot, World};

const GAME: &str = "game name=clock\nrule on=tick every=0.1 do=add:time:1\n";

fn steps(world: &mut World, count: usize) {
    for _ in 0..count {
        world.step(&Inputs::default()).unwrap();
    }
}

#[test]
fn a_save_written_as_json_restores_a_fresh_world_exactly() {
    let definition = GameDef::parse(GAME).unwrap();
    let mut played = World::new(&definition, 7);
    steps(&mut played, 90);
    let saved = played.snapshot().to_json();
    assert!(saved.starts_with('{') && saved.len() > 20, "{saved}");
    // A new world, as after reloading the page, takes the save.
    let mut loaded = World::new(&definition, 7);
    assert_ne!(
        loaded.hash(),
        played.hash(),
        "the fresh world has not played"
    );
    loaded
        .restore(&Snapshot::from_json(&saved).unwrap())
        .unwrap();
    assert_eq!(
        loaded.hash(),
        played.hash(),
        "the save restores the whole state"
    );
    // And both carry on the same way: the random stream, timers and variables were saved too.
    steps(&mut played, 60);
    steps(&mut loaded, 60);
    assert_eq!(loaded.hash(), played.hash());
}

#[test]
fn a_save_from_another_game_or_plain_text_is_refused() {
    let definition = GameDef::parse(GAME).unwrap();
    let other = GameDef::parse("game name=other\nrule on=start do=win\n").unwrap();
    let mut world = World::new(&definition, 1);
    steps(&mut world, 5);
    let foreign = World::new(&other, 1).snapshot().to_json();
    assert!(
        world
            .restore(&Snapshot::from_json(&foreign).unwrap())
            .is_err(),
        "a different game's save does not fit"
    );
    assert!(Snapshot::from_json("not a save").is_err());
    assert!(Snapshot::from_json("{}").is_err());
}
