//! `effect:anim:NAME@selector`: a game rule tells the host to trigger an animation on an entity (a jump, an attack, a hit reaction). The host
//! raises that trigger on the entity's animation state machine. `ve-game` only reports the request, like every effect. Written before the
//! body; do not weaken.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_game::{EffectKind, GameDef, Inputs, World};

fn play(text: &str) -> (World, Vec<ve_game::Effect>) {
    let definition = GameDef::parse(text).unwrap();
    let mut world = World::new(&definition, 1);
    let outcome = world.step(&Inputs::default()).unwrap();
    (world, outcome.effects)
}

#[test]
fn an_anim_effect_names_the_trigger_and_the_entity() {
    let (world, effects) = play(
        "game name=a\nkind hero size=0.5 tags=hero\nspawn hero at=0,0,0\nrule on=start do=effect:anim:jump@tag:hero\n",
    );
    assert_eq!(effects.len(), 1, "{effects:?}");
    assert_eq!(effects[0].kind, EffectKind::Anim);
    assert_eq!(effects[0].name, "jump");
    assert_eq!(effects[0].target, Some(world.entities()[0].id));
}

#[test]
fn a_selector_that_matches_several_entities_triggers_each_of_them() {
    let (world, effects) = play(
        "game name=a\nkind guard size=0.5 tags=guard\nspawn guard at=0,0,0\nspawn guard at=3,0,0\nrule on=start do=effect:anim:alert@tag:guard\n",
    );
    let mut targets: Vec<_> = effects.iter().map(|e| e.target).collect();
    targets.sort();
    let mut ids: Vec<_> = world.entities().iter().map(|e| Some(e.id)).collect();
    ids.sort();
    assert_eq!(targets, ids);
    assert!(
        effects
            .iter()
            .all(|e| e.kind == EffectKind::Anim && e.name == "alert")
    );
}

#[test]
fn without_a_selector_the_trigger_has_no_target() {
    let (_, effects) = play("game name=a\nrule on=start do=effect:anim:wave\n");
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].kind, EffectKind::Anim);
    assert_eq!(effects[0].target, None);
}

#[test]
fn an_anim_effect_needs_a_name_and_a_valid_selector() {
    assert!(
        GameDef::parse("game name=a\nrule on=start do=effect:anim:@a\n").is_err(),
        "no name"
    );
    assert!(
        GameDef::parse("game name=a\nrule on=start do=effect:anim:jump@bogus\n").is_err(),
        "a bad selector"
    );
    assert!(
        GameDef::parse("game name=a\nrule on=start do=effect:anim\n").is_err(),
        "no name at all"
    );
}

#[test]
fn the_other_effect_kinds_still_work() {
    let (_, effects) = play(
        "game name=a\nrule on=start do=effect:sfx:ding;effect:particles:puff;effect:camera_shake:small;effect:message:hello\n",
    );
    let kinds: Vec<_> = effects.iter().map(|e| e.kind.clone()).collect();
    assert_eq!(
        kinds,
        vec![
            EffectKind::Sfx,
            EffectKind::Particles,
            EffectKind::CameraShake,
            EffectKind::Message
        ]
    );
}

#[test]
fn an_anim_effect_is_deterministic_and_survives_a_snapshot() {
    let text = "game name=a\nkind hero size=0.5 tags=hero\nspawn hero at=0,0,0\nrule on=start do=effect:anim:jump@tag:hero\n";
    let (_, first) = play(text);
    let (_, second) = play(text);
    assert_eq!(first, second);
}
