//! JavaScript behaviours inside the fixed step: they hear the world's events, change entities and variables, raise events rules hear,
//! keep `state` across saves, are deterministic, and are stopped (not hung) when they run away.
#![expect(
    clippy::unwrap_used,
    clippy::float_cmp,
    reason = "fixtures unwrap definitions that are valid by construction and compare values that only ever hold whole numbers"
)]
use ve_game::{GameDef, GameError, Inputs, ScriptDef, World};

fn game(script: &str) -> GameDef {
    let mut text = String::from(
        "game name=duel\nvar score=0 hits=0\nkind hero size=0.5 speed=4 tags=hero\nkind coin size=0.4 tags=pickup\n\
         spawn hero at=0,0,0\nspawn coin at=0.2,0,0\nrule on=emit:scored do=add:hits:1\n",
    );
    text.push_str(&ScriptDef::new("main", script).to_text());
    text.push('\n');
    GameDef::parse(&text).unwrap()
}

fn run(world: &mut World, steps: usize) {
    for _ in 0..steps {
        world.step(&Inputs::default()).unwrap();
    }
}

#[test]
fn a_script_hears_touch_and_changes_the_world() {
    let def = game(
        "on('touch', { a: 'tag:hero', b: 'tag:pickup' }, (e) => { game.add('score', 1); e.b.destroy(); game.emit('scored'); });",
    );
    let mut world = World::new(&def, 7);
    run(&mut world, 2);
    assert_eq!(world.vars()["score"], 1.0);
    assert_eq!(
        world.vars()["hits"],
        1.0,
        "a rule heard the event the script emitted"
    );
    assert!(world.entities().iter().all(|e| e.kind != "coin"));
    assert!(world.take_script_log().is_empty());
}

#[test]
fn timers_start_and_entity_writes_work() {
    let def = game(
        "on('start', () => { state.n = 0; }); every(0.5, () => { state.n += 1; game.set('score', state.n); }); \
         on('tick', () => { const h = find('kind:hero')[0]; h.pos = [h.pos[0] + 0.1, 0, 0]; h.add('mood', 1); });",
    );
    let mut world = World::new(&def, 7);
    run(&mut world, 60);
    assert_eq!(
        world.vars()["score"],
        2.0,
        "every(0.5) fired twice in a second"
    );
    let hero = world.entities().iter().find(|e| e.kind == "hero").unwrap();
    assert!((hero.position[0] - 6.0).abs() < 1e-9);
    assert_eq!(hero.props["mood"], 60.0);
}

#[test]
fn spawning_and_random_are_deterministic() {
    let def = game(
        "on('tick', (e) => { if (e.tick % 10 === 0) spawn('coin', [Math.random() * 10, 0, Math.random() * 10]); });",
    );
    let (mut a, mut b) = (World::new(&def, 42), World::new(&def, 42));
    run(&mut a, 120);
    run(&mut b, 120);
    assert_eq!(a.hash(), b.hash());
    assert!(a.entities().len() > 10);
    let mut c = World::new(&def, 43);
    run(&mut c, 120);
    assert_ne!(a.hash(), c.hash(), "another seed gives another game");
}

#[test]
fn state_survives_a_snapshot_and_restore() {
    let def = game(
        "on('start', () => { state.count = 0; }); on('tick', () => { state.count += 1; game.set('score', state.count); });",
    );
    let mut world = World::new(&def, 1);
    run(&mut world, 30);
    let saved = world.snapshot();
    let at_save = world.hash();
    run(&mut world, 30);
    assert_eq!(world.vars()["score"], 60.0);
    world.restore(&saved).unwrap();
    assert_eq!(world.hash(), at_save);
    run(&mut world, 30);
    assert_eq!(
        world.vars()["score"],
        60.0,
        "the counter continued from 30, not from 60"
    );
    let reloaded = ve_game::Snapshot::from_json(&saved.to_json()).unwrap();
    let mut other = World::new(&def, 1);
    other.restore(&reloaded).unwrap();
    assert_eq!(
        other.hash(),
        at_save,
        "a save written to text and read back restores the scripts' state too"
    );
}

#[test]
fn a_runaway_script_is_stopped_and_the_step_rolled_back() {
    let def = game("on('tick', () => { for (;;) {} });");
    let mut world = World::new(&def, 1);
    let before = world.hash();
    let error = world.step(&Inputs::default()).unwrap_err();
    assert!(
        matches!(error, GameError::Script(ref m) if m.contains("work budget")),
        "{error}"
    );
    assert_eq!(world.hash(), before, "the failed step changed nothing");
}

#[test]
fn a_throwing_handler_is_reported_and_the_game_goes_on() {
    let def = game(
        "on('tick', () => { throw new Error('boom'); }); on('tick', () => { game.add('score', 1); });",
    );
    let mut world = World::new(&def, 1);
    run(&mut world, 3);
    assert_eq!(world.vars()["score"], 3.0, "the next handler still ran");
    let log = world.take_script_log();
    assert!(
        log.iter()
            .any(|line| line.contains("boom") && line.contains("main")),
        "{log:?}"
    );
}

#[test]
fn scripts_cannot_reach_a_clock_or_the_host() {
    let def = game(
        "log(typeof Date, typeof __host, typeof fetch, typeof performance); on('tick', () => {});",
    );
    let mut world = World::new(&def, 1);
    assert_eq!(
        world.take_script_log(),
        vec!["undefined undefined undefined undefined".to_owned()]
    );
}

#[test]
fn a_script_that_does_not_parse_is_reported_not_fatal() {
    let def = game("on('tick', () => {");
    let mut world = World::new(&def, 1);
    run(&mut world, 2);
    let log = world.take_script_log();
    assert!(
        log.iter().any(|line| line.contains("did not start")),
        "{log:?}"
    );
}

#[test]
fn the_definition_text_round_trips_with_scripts() {
    let def = game("on('start', () => {\n  log(\"hi\\n\");\n});\n// a comment # not a rule\n");
    let again = GameDef::parse(&def.to_text()).unwrap();
    assert_eq!(again.scripts, def.scripts);
}
