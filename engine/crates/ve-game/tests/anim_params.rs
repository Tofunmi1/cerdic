//! `effect:anim_set:NAME:EXPR[@SELECTOR]` and `effect:anim_flag:NAME:EXPR[@SELECTOR]`: a game rule sets a parameter of an animation state
//! machine from game state (a number from an expression, or a true/false from one). This is how input and gameplay drive animation:
//! `speed` from how fast the hero moves, `grounded` from physics, `armed` from a variable. `ve-game` only reports the request, as every
//! effect; the host applies it. Written before the body; do not weaken.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_game::{Effect, EffectKind, GameDef, Inputs, World};

fn play(text: &str) -> (World, Vec<Effect>) {
    let definition = GameDef::parse(text).unwrap();
    let mut world = World::new(&definition, 1);
    let outcome = world.step(&Inputs::default()).unwrap();
    (world, outcome.effects)
}

const HERO: &str = "game name=a\nkind hero size=0.5 tags=hero\nspawn hero at=0,0,0\n";

#[test]
fn a_number_parameter_is_set_from_a_literal() {
    let (world, effects) = play(&format!(
        "{HERO}rule on=start do=effect:anim_set:speed:2.5@tag:hero\n"
    ));
    assert_eq!(effects.len(), 1, "{effects:?}");
    assert_eq!(effects[0].kind, EffectKind::AnimSet);
    assert_eq!(effects[0].name, "speed");
    assert_eq!(effects[0].value, Some(2.5));
    assert_eq!(effects[0].target, Some(world.entities()[0].id));
}

#[test]
fn the_value_is_an_expression_over_game_variables() {
    let (_, effects) =
        play("game name=a\nvar gait=0.5\nrule on=start do=effect:anim_set:speed:gait*4+1\n");
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].value, Some(3.0));
    assert_eq!(effects[0].target, None, "no selector: every machine");
}

#[test]
fn speed_and_vy_name_the_targets_own_movement() {
    // The hero is moving at (3, 0, 4): a horizontal speed of 5; and rising at 2 units a second.
    let text = format!(
        "{HERO}rule on=start a=tag:hero do=setvel:a:3,2,4;effect:anim_set:speed:speed@a;effect:anim_set:vy:vy@a\n"
    );
    let (_, effects) = play(&text);
    let by_name = |name: &str| {
        effects
            .iter()
            .find(|e| e.name == name)
            .and_then(|e| e.value)
    };
    assert!(
        (by_name("speed").unwrap() - 5.0).abs() < 1e-9,
        "{effects:?}"
    );
    assert!((by_name("vy").unwrap() - 2.0).abs() < 1e-9, "{effects:?}");
}

#[test]
fn a_flag_is_true_for_any_non_zero_value() {
    let (_, effects) = play(&format!(
        "{HERO}rule on=start do=effect:anim_flag:grounded:1@tag:hero;effect:anim_flag:armed:0@tag:hero;effect:anim_flag:moving:speed>0.5@tag:hero\n"
    ));
    assert_eq!(effects.len(), 3, "{effects:?}");
    assert!(effects.iter().all(|e| e.kind == EffectKind::AnimFlag));
    let value = |name: &str| {
        effects
            .iter()
            .find(|e| e.name == name)
            .and_then(|e| e.value)
    };
    assert_eq!(value("grounded"), Some(1.0));
    assert_eq!(value("armed"), Some(0.0));
    assert_eq!(
        value("moving"),
        Some(0.0),
        "a hero that stands still is not moving"
    );
}

#[test]
fn a_selector_that_matches_several_entities_sets_each_of_them() {
    let (world, effects) = play(
        "game name=a\nkind guard size=0.5 tags=guard\nspawn guard at=0,0,0\nspawn guard at=3,0,0\nrule on=start do=effect:anim_set:speed:1@tag:guard\n",
    );
    let mut targets: Vec<_> = effects.iter().map(|e| e.target).collect();
    targets.sort();
    let mut ids: Vec<_> = world.entities().iter().map(|e| Some(e.id)).collect();
    ids.sort();
    assert_eq!(targets, ids);
}

#[test]
fn it_works_every_tick_from_a_rule_with_a_condition() {
    let definition = GameDef::parse(&format!(
        "{HERO}rule on=tick do=effect:anim_set:speed:3@tag:hero\n"
    ))
    .unwrap();
    let mut world = World::new(&definition, 1);
    let mut seen = 0;
    for _ in 0..5 {
        seen += world
            .step(&Inputs::default())
            .unwrap()
            .effects
            .iter()
            .filter(|e| e.kind == EffectKind::AnimSet)
            .count();
    }
    assert_eq!(seen, 5, "one request per tick");
}

#[test]
fn bad_forms_are_refused_at_parse_time() {
    for bad in [
        "effect:anim_set:speed",
        "effect:anim_set::2",
        "effect:anim_set:speed:",
        "effect:anim_set:speed:2**",
        "effect:anim_set:speed:2@bogus",
        "effect:anim_flag:grounded",
        "effect:anim_flag::1",
        "effect:anim_flag:grounded:1@bogus",
    ] {
        let text = format!("game name=a\nrule on=start do={bad}\n");
        assert!(GameDef::parse(&text).is_err(), "{bad} should be refused");
    }
}

#[test]
fn the_other_effects_carry_no_value_and_the_anim_trigger_still_works() {
    let (_, effects) = play(&format!(
        "{HERO}rule on=start do=effect:sfx:ding;effect:anim:jump@tag:hero\n"
    ));
    assert_eq!(effects.len(), 2);
    assert!(effects.iter().all(|e| e.value.is_none()), "{effects:?}");
    assert_eq!(effects[1].kind, EffectKind::Anim);
}

#[test]
fn the_request_is_deterministic() {
    let text = format!("{HERO}rule on=start do=effect:anim_set:speed:2@tag:hero\n");
    assert_eq!(play(&text).1, play(&text).1);
}
