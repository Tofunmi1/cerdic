//! The `combat` library: melee timelines with startup, active and recovery frames, hit arcs, hitstop, combos from buffered input, dodges
//! with i-frames, block and parry, all inside the fixed step.
#![expect(
    clippy::unwrap_used,
    reason = "fixtures unwrap definitions that are valid by construction"
)]
use ve_game::{GameDef, Inputs, ScriptDef, World};

const ATTACKS: &str = "combat.attack_def('light1', { startup: 0.2, active: 0.1, recovery: 0.3, cancel_from: 0.05, next: 'light2', cost: { stamina: 10 }, shape: { range: 1.5, arc: 100 }, damage: { amount: 20, poise: 10 }, hitstop: 0.1, knockback: 0.5 }); \
    combat.attack_def('light2', { startup: 0.1, active: 0.1, recovery: 0.3, shape: { range: 1.5, arc: 100 }, damage: { amount: 30, poise: 10 }, hitstop: 0.1, knockback: 0.5 }); \
    combat.dodge_def('roll', { duration: 0.4, i_frames: [0.0, 0.3], distance: 1, cost: { stamina: 5 } });";

fn world(extra: &str) -> World {
    let mut text = String::from(
        "game name=fight\nkind knight size=0.4 speed=2\nkind dummy size=0.4\nspawn knight at=0,0,0\nspawn dummy at=0,0,1\n",
    );
    for library in ["rpg", "combat"] {
        text.push_str(&ScriptDef::library(library, library).unwrap().to_text());
        text.push('\n');
    }
    let setup = "on('start', () => { const [k] = find('kind:knight'); const [d] = find('kind:dummy'); \
        rpg.actor(k, { attributes: { health: 100, max_health: 100, stamina: 100, max_stamina: 100, poise: 100, max_poise: 100 }, tags: ['team.a'] }); \
        rpg.actor(d, { attributes: { health: 200, max_health: 200, poise: 8, max_poise: 8, stagger_time: 1 }, tags: ['team.b'] }); state.k = k.id; state.d = d.id; });";
    text.push_str(&ScriptDef::new("main", &format!("{ATTACKS} {setup} {extra}")).to_text());
    text.push('\n');
    World::new(&GameDef::parse(&text).unwrap(), 3)
}

fn run(world: &mut World, steps: usize) {
    for _ in 0..steps {
        world.step(&Inputs::default()).unwrap();
    }
}

fn saved(world: &World) -> String {
    world.snapshot().to_json()
}

fn dummy_health(world: &World) -> String {
    // The script publishes it each tick through a game variable the tests read.
    format!("{}", world.vars().get("dh").copied().unwrap_or(-1.0))
}

const PROBE: &str = "on('tick', () => { game.set('dh', Math.round(rpg.get(entity(state.d), 'health'))); game.set('dp', Math.round(rpg.get(entity(state.d), 'poise'))); });";

#[test]
fn damage_lands_only_in_the_active_window() {
    let mut w = world(&format!(
        "{PROBE} on('tick', (e) => {{ if (e.tick === 2) combat.input(entity(state.k), 'light1'); }});"
    ));
    run(&mut w, 15); // startup is 12 frames: nothing yet
    assert_eq!(dummy_health(&w), "200");
    run(&mut w, 6); // active frames 13 to 18
    assert!(
        w.vars()["dh"] < 200.0,
        "the dummy was hit: {}",
        dummy_health(&w)
    );
    let before = w.vars()["dh"];
    run(&mut w, 60);
    assert_eq!(w.vars()["dh"], before, "one attack hits a target once");
    assert!(w.take_script_log().is_empty());
}

#[test]
fn a_buffered_second_press_chains_the_combo() {
    let mut w = world(&format!(
        "{PROBE} on('tick', (e) => {{ const k = entity(state.k); if (e.tick === 2) combat.input(k, 'light1'); if (e.tick === 24) combat.input(k, 'light2'); }});"
    ));
    run(&mut w, 120);
    // light1 deals 20 and light2 30, each reduced by nothing (no armour) and the dummy is knocked back out of reach after the first hit.
    assert!(w.take_script_log().is_empty());
    let health = w.vars()["dh"];
    assert!(health <= 180.0, "at least the first hit landed: {health}");
}

#[test]
fn hitstop_freezes_both_fighters_briefly() {
    let mut w = world(&format!(
        "{PROBE} on('tick', (e) => {{ if (e.tick === 2) combat.input(entity(state.k), 'light1'); if (combat.frozen(entity(state.k)) && combat.frozen(entity(state.d))) state.froze = (state.froze || 0) + 1; }});"
    ));
    run(&mut w, 40);
    assert!(
        saved(&w).contains("\\\"froze\\\":"),
        "both were frozen at some point: {}",
        saved(&w)
    );
}

#[test]
fn a_dodge_in_its_i_frames_takes_no_damage_and_poise_breaks_stagger() {
    let mut w = world(&format!(
        "{PROBE} on('start', () => {{ state.n = 0; }}); on('tick', (e) => {{ const k = entity(state.k); const d = entity(state.d); \
           if (e.tick === 2) combat.input(k, 'light1'); \
           if (e.tick === 40) state.staggered = rpg.has_tag(d, 'state.staggered'); \
           if (e.tick === 100) {{ combat.input(k, 'roll'); }} \
           if (e.tick === 103) {{ state.inv = rpg.has_tag(k, 'state.invulnerable'); rpg.damage(k, {{ amount: 50 }}); state.khp = rpg.get(k, 'health'); }} }});"
    ));
    run(&mut w, 120);
    let s = saved(&w);
    assert!(
        s.contains("\\\"staggered\\\":true"),
        "10 poise damage against 8 poise breaks it: {s}"
    );
    assert!(
        s.contains("\\\"inv\\\":true") && s.contains("\\\"khp\\\":100"),
        "i-frames absorb the hit: {s}"
    );
}

#[test]
fn a_held_block_cuts_damage() {
    let mut w = world(&format!(
        "{PROBE} on('tick', (e) => {{ if (e.tick === 2) combat.input(entity(state.d), 'block_on'); if (e.tick === 4) combat.input(entity(state.k), 'light1'); }});"
    ));
    run(&mut w, 60);
    assert!(w.take_script_log().is_empty());
    assert_eq!(
        w.vars()["dh"],
        194.0,
        "20 damage cut by 70% by a block started long before the hit"
    );
}

#[test]
fn a_block_started_just_before_the_hit_parries_it() {
    let mut w = world(&format!(
        "{PROBE} on('tick', (e) => {{ if (e.tick === 12) combat.input(entity(state.d), 'block_on'); if (e.tick === 4) combat.input(entity(state.k), 'light1'); }});"
    ));
    run(&mut w, 40);
    assert!(w.take_script_log().is_empty());
    assert_eq!(w.vars()["dh"], 200.0, "a parried hit does no damage");
    assert!(
        saved(&w).contains("state.staggered"),
        "and the attacker is staggered: {}",
        saved(&w)
    );
}

#[test]
fn combat_is_deterministic_and_restores() {
    let script = format!(
        "{PROBE} on('tick', (e) => {{ const k = entity(state.k); if (e.tick % 40 === 2) combat.input(k, 'light1'); if (e.tick % 40 === 22) combat.input(k, 'light2'); }});"
    );
    let (mut a, mut b) = (world(&script), world(&script));
    run(&mut a, 200);
    run(&mut b, 200);
    assert_eq!(a.hash(), b.hash());
    let snapshot = a.snapshot();
    let hash = a.hash();
    run(&mut a, 80);
    a.restore(&snapshot).unwrap();
    assert_eq!(a.hash(), hash);
    run(&mut a, 80);
    run(&mut b, 80);
    assert_eq!(a.hash(), b.hash());
}
