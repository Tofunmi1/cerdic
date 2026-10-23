//! Rule fire counts participate in world snapshots and restores.
use ve_game::{GameDef, Inputs, World};

#[test]
fn restoring_a_snapshot_restores_rule_fire_counts() -> Result<(), String> {
    let definition = GameDef::parse("game name=counter\nrule on=start do=win\n")
        .map_err(|_| "the game fixture should parse".to_owned())?;
    let mut world = World::new(&definition, 4);
    let before = world.snapshot();
    world
        .step(&Inputs::default())
        .map_err(|error| error.to_string())?;
    assert_eq!(world.rule_fires(), &[1]);
    world.restore(&before).map_err(|error| error.to_string())?;
    assert_eq!(world.rule_fires(), &[0]);
    world
        .step(&Inputs::default())
        .map_err(|error| error.to_string())?;
    assert_eq!(world.rule_fires(), &[1]);
    Ok(())
}
