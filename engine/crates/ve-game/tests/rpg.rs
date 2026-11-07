//! The `rpg` standard library: attributes with modifiers, tags, effects, abilities and the damage pipeline, running as a script
//! inside the fixed step.
#![expect(
    clippy::unwrap_used,
    clippy::float_cmp,
    reason = "fixtures unwrap definitions that are valid by construction; attributes are compared where they are exact"
)]
use ve_game::{GameDef, Inputs, ScriptDef, World};

fn world(script: &str) -> World {
    let mut text = String::from(
        "game name=arena\nvar out=0\nkind fighter size=0.5 speed=3 tags=fighter\nspawn fighter at=0,0,0\nspawn fighter at=5,0,0\n\
         rule on=emit:rpg_died do=add:out:1\n",
    );
    text.push_str(&ScriptDef::library("rpg", "rpg").unwrap().to_text());
    text.push('\n');
    text.push_str(&ScriptDef::new("main", script).to_text());
    text.push('\n');
    World::new(&GameDef::parse(&text).unwrap(), 5)
}

fn run(world: &mut World, steps: usize) {
    for _ in 0..steps {
        world.step(&Inputs::default()).unwrap();
    }
}

const SETUP: &str = "on('start', () => { const [a, b] = find('kind:fighter'); \
    rpg.actor(a, { attributes: { health: 100, max_health: 100, stamina: 50, max_stamina: 50, armor: 0, strength: 10, poise: 30, max_poise: 30, stagger_time: 1 }, tags: ['team.a'] }); \
    rpg.actor(b, { attributes: { health: 100, max_health: 100, armor: 100, strength: 10 }, resist: { fire: 0.5 }, tags: ['team.b'] }); \
    state.a = a.id; state.b = b.id; });";

fn with(extra: &str) -> World {
    world(&format!("{SETUP} {extra}"))
}

#[test]
fn modifiers_stack_and_expire_with_their_effect() {
    let mut w = with(
        "rpg.effect_def('rage', { duration: 1, modifiers: [{ attr: 'strength', op: 'add', value: 5 }, { attr: 'strength', op: 'mul', value: 2 }], tags: ['state.raging'] }); \
         on('tick', (e) => { const a = entity(state.a); if (e.tick === 2) { rpg.apply(a, 'rage', a); state.during = rpg.get(a, 'strength'); state.tagged = rpg.has_tag(a, 'state'); } \
                             if (e.tick === 100) { state.after = rpg.get(a, 'strength'); state.tag_after = rpg.has_tag(a, 'state.raging'); } });",
    );
    run(&mut w, 100);
    let log = w.take_script_log();
    assert!(log.is_empty(), "{log:?}");
    let saved = w.snapshot().to_json();
    assert!(
        saved.contains("\\\"during\\\":30"),
        "(10 + 5) x 2 while raging: {saved}"
    );
    assert!(
        saved.contains("\\\"tagged\\\":true"),
        "a tag under 'state' matches 'state': {saved}"
    );
    assert!(
        saved.contains("\\\"after\\\":10") && saved.contains("\\\"tag_after\\\":false"),
        "back to base after a second: {saved}"
    );
}

#[test]
fn damage_uses_armour_resistance_block_and_kills_once() {
    let mut w = with(
        "on('tick', (e) => { const [a, b] = [entity(state.a), entity(state.b)]; \
           if (e.tick === 2) { state.plain = rpg.damage(a, { amount: 20, source: b }); \
                               state.armoured = rpg.damage(b, { amount: 20, source: a }); \
                               state.fire = rpg.damage(a, { amount: 20, type: 'fire' }); \
                               rpg.add_tag(a, 'state.blocking', 'test', 0); state.blocked = rpg.damage(a, { amount: 20, source: b }); } \
           if (e.tick === 4) { rpg.damage(b, { amount: 1000, source: a }); rpg.damage(b, { amount: 1000, source: a }); } });",
    );
    run(&mut w, 6);
    assert!(w.take_script_log().is_empty());
    let saved = w.snapshot().to_json();
    assert!(saved.contains("\\\"plain\\\":20"), "{saved}");
    assert!(
        saved.contains("\\\"armoured\\\":10"),
        "armour 100 halves physical damage: {saved}"
    );
    assert!(saved.contains("\\\"fire\\\":20"), "{saved}");
    assert!(
        saved.contains("\\\"blocked\\\":6"),
        "block cuts 70%: {saved}"
    );
    assert_eq!(
        w.vars()["out"],
        1.0,
        "death is raised once even when hit twice"
    );
}

#[test]
fn abilities_pay_cost_wait_for_cooldown_and_respect_tags() {
    let mut w = with(
        "rpg.ability_def('slash', { cost: { stamina: 20 }, cooldown: 1, blocked_by: ['state.stunned'], run: (user, target) => { state.slashes = (state.slashes || 0) + 1; } }); \
         on('tick', (e) => { const a = entity(state.a); \
           if (e.tick === 2) state.first = rpg.use(a, 'slash'); \
           if (e.tick === 3) { state.again = rpg.use(a, 'slash'); state.why = rpg.last_refusal; } \
           if (e.tick === 70) state.later = rpg.use(a, 'slash'); \
           if (e.tick === 140) { rpg.add_tag(a, 'state.stunned', 't', 0); state.stunned = rpg.use(a, 'slash'); state.why2 = rpg.last_refusal; } \
           if (e.tick === 141) state.stamina = rpg.get(a, 'stamina'); });",
    );
    run(&mut w, 141);
    assert!(w.take_script_log().is_empty());
    let saved = w.snapshot().to_json();
    assert!(saved.contains("\\\"first\\\":true"), "{saved}");
    assert!(
        saved.contains("\\\"again\\\":false") && saved.contains("cooldown"),
        "{saved}"
    );
    assert!(saved.contains("\\\"later\\\":true"), "{saved}");
    assert!(
        saved.contains("\\\"stunned\\\":false") && saved.contains("blocked by state.stunned"),
        "{saved}"
    );
    assert!(saved.contains("\\\"slashes\\\":2"), "{saved}");
    assert!(
        saved.contains("\\\"stamina\\\":10") || saved.contains("\\\"stamina\\\":1"),
        "two slashes cost 40 of 50 (regen aside): {saved}"
    );
}

#[test]
fn periodic_effects_and_poise_stagger_work() {
    let mut w = with(
        "rpg.effect_def('burn', { duration: 3, period: 1, tags: ['state.burning'], on_tick: (target, source) => rpg.damage(target, { amount: 5, type: 'fire' }) }); \
         on('tick', (e) => { const a = entity(state.a); \
           if (e.tick === 2) rpg.apply(a, 'burn', null); \
           if (e.tick === 5) { rpg.damage(a, { amount: 1, poise: 20 }); rpg.damage(a, { amount: 1, poise: 20, source: entity(state.b) }); } \
           if (e.tick === 6) state.staggered = rpg.has_tag(a, 'state.staggered'); \
           if (e.tick === 200) { state.health = rpg.get(a, 'health'); state.burning = rpg.has_tag(a, 'state.burning'); state.staggered_later = rpg.has_tag(a, 'state.staggered'); } });",
    );
    run(&mut w, 200);
    assert!(w.take_script_log().is_empty());
    let saved = w.snapshot().to_json();
    assert!(
        saved.contains("\\\"staggered\\\":true") && saved.contains("\\\"staggered_later\\\":false"),
        "{saved}"
    );
    assert!(saved.contains("\\\"burning\\\":false"), "{saved}");
    assert!(
        saved.contains("\\\"health\\\":83"),
        "3 burn ticks of 5 plus two 1-damage hits: {saved}"
    );
}

#[test]
fn the_rpg_state_is_saved_restored_and_deterministic() {
    let script = "on('tick', (e) => { const a = entity(state.a); if (e.tick % 20 === 0) rpg.damage(a, { amount: 3 + Math.floor(Math.random() * 5) }); });";
    let (mut x, mut y) = (with(script), with(script));
    run(&mut x, 100);
    run(&mut y, 100);
    assert_eq!(x.hash(), y.hash());
    let saved = x.snapshot();
    let hash = x.hash();
    run(&mut x, 60);
    assert_ne!(x.hash(), hash);
    x.restore(&saved).unwrap();
    assert_eq!(
        x.hash(),
        hash,
        "health, modifiers and cooldowns come back with the snapshot"
    );
    run(&mut x, 60);
    run(&mut y, 60);
    assert_eq!(
        x.hash(),
        y.hash(),
        "and the game goes on exactly as it would have"
    );
}
