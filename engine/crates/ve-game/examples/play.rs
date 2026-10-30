//! Runs a small deterministic game definition.
use std::io::Write;
use ve_game::{GameDef, Inputs, World};
fn main() -> std::io::Result<()> {
    let source = "game name=coins\nvar score=0\nkind player size=0.5 speed=4 tags=hero\nspawn player at=0,0,0";
    let Ok(def) = GameDef::parse(source) else {
        return Ok(());
    };
    let mut world = World::new(&def, 7);
    let _ = world.step(&Inputs::default());
    writeln!(
        std::io::stdout().lock(),
        "state={:?} hash={:016x}",
        world.vars(),
        world.hash()
    )?;
    Ok(())
}
