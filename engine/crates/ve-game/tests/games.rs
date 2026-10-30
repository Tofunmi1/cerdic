//! The rules core as a player sees it: whole games written as text and played by scripted policies, plus parser errors, budgets,
//! determinism, host events and validation. Written before the bodies; do not weaken.
#![expect(
    clippy::unwrap_used,
    clippy::float_cmp,
    reason = "fixtures unwrap definitions that are valid by construction and compare variables that only ever hold whole numbers"
)]
use std::collections::BTreeMap;
use ve_game::{EffectKind, GameDef, GameError, HostEvent, Inputs, Status, World};

const COINS: &str = "game name=coins
var score=0 lives=3 time=0
kind player size=0.5 speed=4 tags=hero
kind coin size=0.4 tags=pickup
kind spike size=0.5 tags=hazard
spawn player at=0,0,0
spawn coin at=3,0,2 count=8 spread=6
input move axis=x,z target=tag:hero
input jump key=press
rule on=touch a=tag:hero b=tag:pickup do=add:score:1;destroy:b;effect:sfx:pickup@b
rule on=touch a=tag:hero b=tag:hazard do=add:lives:-1;destroy:b;effect:particles:explosion@b
rule on=tick every=1.0 do=add:time:1
rule on=var:lives when=lives<=0 do=lose
goal win when=score>=8
goal lose when=time>=60
";

fn parse(text: &str) -> GameDef {
    GameDef::parse(text).unwrap()
}

fn axes(x: f64, z: f64) -> Inputs {
    Inputs {
        axes: BTreeMap::from([("move.x".to_owned(), x), ("move.z".to_owned(), z)]),
        ..Inputs::default()
    }
}

/// Steers the hero straight at the nearest entity of a kind (a bot's whole brain).
fn steer(world: &World, hero_kind: &str, target_kind: &str) -> Inputs {
    let hero = world.entities().iter().find(|e| e.kind == hero_kind);
    let goal = world
        .entities()
        .iter()
        .filter(|e| e.kind == target_kind)
        .min_by(|a, b| {
            let d = |e: &ve_game::Entity| {
                hero.map_or(0.0, |h| {
                    (e.position[0] - h.position[0]).powi(2)
                        + (e.position[2] - h.position[2]).powi(2)
                })
            };
            d(a).total_cmp(&d(b))
        });
    match (hero, goal) {
        (Some(h), Some(g)) => {
            let (dx, dz) = (g.position[0] - h.position[0], g.position[2] - h.position[2]);
            let len = (dx * dx + dz * dz).sqrt().max(1e-9);
            axes(dx / len, dz / len)
        }
        _ => Inputs::default(),
    }
}

#[test]
fn the_coin_game_is_won_by_a_bot_that_walks_to_the_nearest_coin() {
    let def = parse(COINS);
    let mut world = World::new(&def, 7);
    let mut status = Status::Playing;
    for _ in 0..3000 {
        let inputs = steer(&world, "player", "coin");
        status = world.step(&inputs).unwrap().status;
        if status != Status::Playing {
            break;
        }
    }
    assert_eq!(status, Status::Won, "vars {:?}", world.vars());
    assert_eq!(world.vars()["score"], 8.0);
    assert_eq!(
        world.entities().iter().filter(|e| e.kind == "coin").count(),
        0,
        "every coin was destroyed on pick-up"
    );
}

#[test]
fn the_coin_game_is_lost_by_standing_still_until_the_clock_runs_out() {
    let def = parse(COINS);
    let mut world = World::new(&def, 7);
    let mut last = Status::Playing;
    let mut ticks = 0;
    while last == Status::Playing && ticks < 4000 {
        last = world.step(&Inputs::default()).unwrap().status;
        ticks += 1;
    }
    assert_eq!(last, Status::Lost);
    assert!(
        (3590..=3670).contains(&ticks),
        "60 seconds at 60 steps a second, lost after {ticks} steps"
    );
    assert!(world.vars()["time"] >= 60.0);
}

#[test]
fn touch_fires_once_when_an_overlap_starts_not_every_step_and_again_on_re_entry() {
    let def = parse(
        "game name=once
var score=0
kind hero size=0.5 speed=4 tags=hero
kind gem size=0.4 tags=pickup
spawn hero at=0,0,0
spawn gem at=0.3,0,0
input move axis=x,z target=tag:hero
rule on=touch a=tag:hero b=tag:pickup do=add:score:1
",
    );
    let mut world = World::new(&def, 1);
    for _ in 0..30 {
        world.step(&Inputs::default()).unwrap();
    }
    assert_eq!(
        world.vars()["score"],
        1.0,
        "standing in the gem counts once"
    );
    for _ in 0..30 {
        world.step(&axes(1.0, 0.0)).unwrap();
    }
    for _ in 0..30 {
        world.step(&axes(-1.0, 0.0)).unwrap();
    }
    assert_eq!(
        world.vars()["score"],
        2.0,
        "leaving and coming back counts again"
    );
}

#[test]
fn destroying_many_touched_entities_in_one_step_is_safe() {
    let def = parse(
        "game name=pile
var score=0
kind hero size=0.5 speed=4 tags=hero
kind coin size=0.4 tags=pickup
spawn hero at=0,0,0
spawn coin at=0,0,0 count=8 spread=0
rule on=touch a=tag:hero b=tag:pickup do=add:score:1;destroy:b
",
    );
    let mut world = World::new(&def, 3);
    world.step(&Inputs::default()).unwrap();
    assert_eq!(world.vars()["score"], 8.0);
    assert_eq!(world.entities().len(), 1, "only the hero is left");
}

#[test]
fn effects_are_returned_to_the_host_not_executed() {
    let def = parse(COINS);
    let mut world = World::new(&def, 7);
    let mut seen = Vec::new();
    for _ in 0..3000 {
        let inputs = steer(&world, "player", "coin");
        let outcome = world.step(&inputs).unwrap();
        seen.extend(outcome.effects);
        if outcome.status != Status::Playing {
            break;
        }
    }
    let pickups: Vec<_> = seen
        .iter()
        .filter(|e| e.kind == EffectKind::Sfx && e.name == "pickup")
        .collect();
    assert_eq!(pickups.len(), 8, "one pick-up sound per coin");
    assert!(
        pickups.iter().all(|e| e.target.is_some()),
        "the sound names the coin it belongs to"
    );
}

#[test]
fn a_pong_like_game_uses_velocity_touch_and_emit() {
    let def = parse(
        "game name=pong
var hits=0
kind paddle size=0.5 tags=paddle
kind ball size=0.3 tags=ball
spawn paddle at=3,0,0
spawn paddle at=-3,0,0
spawn ball at=0,0,0
rule on=start do=setvel:tag:ball:2,0,0
rule on=touch a=tag:ball b=tag:paddle do=add:hits:1;emit:bounce
rule on=emit:bounce do=setvel:tag:ball:0,0,0
rule on=emit:bounce when=hits<=1 do=setvel:tag:ball:-2,0,0
rule on=emit:bounce when=hits==2 do=setvel:tag:ball:2,0,0
rule on=emit:bounce when=hits>=3 do=win
",
    );
    let mut world = World::new(&def, 5);
    let mut status = Status::Playing;
    for _ in 0..1200 {
        status = world.step(&Inputs::default()).unwrap().status;
        if status != Status::Playing {
            break;
        }
    }
    assert_eq!(status, Status::Won, "vars {:?}", world.vars());
    assert_eq!(world.vars()["hits"], 3.0);
}

#[test]
fn a_shooter_spawns_on_timers_and_is_won_by_pressing_shoot() {
    let def = parse(
        "game name=range
var spawned=0 kills=0
kind turret size=0.5 tags=hero
kind target size=0.5 tags=foe
spawn turret at=0,0,0
input shoot key=press
rule on=start do=timer:wave:1.0
rule on=timer:wave when=spawned<5 do=spawn:target@4,0,0;add:spawned:1;timer:wave:1.0
rule on=input:shoot do=destroy:tag:foe;add:kills:1
goal win when=kills>=3
",
    );
    let mut world = World::new(&def, 9);
    let count = |w: &World| w.entities().iter().filter(|e| e.kind == "target").count();
    for _ in 0..58 {
        world.step(&Inputs::default()).unwrap();
    }
    assert_eq!(count(&world), 0, "the first wave is one second away");
    for _ in 0..4 {
        world.step(&Inputs::default()).unwrap();
    }
    assert_eq!(count(&world), 1, "the timer fired after about 60 steps");
    let mut status = Status::Playing;
    for _ in 0..900 {
        let shoot = if count(&world) > 0 {
            Inputs {
                pressed: vec!["shoot".to_owned()],
                ..Inputs::default()
            }
        } else {
            Inputs::default()
        };
        status = world.step(&shoot).unwrap().status;
        if status != Status::Playing {
            break;
        }
    }
    assert_eq!(status, Status::Won, "vars {:?}", world.vars());
}

#[test]
fn the_host_can_raise_named_events_with_participants() {
    let def = parse(
        "game name=host
var opened=0 picked=0
kind lamp size=0.3 tags=item
spawn lamp at=1,0,0
rule on=host:door_opened do=add:opened:1
rule on=host:grabbed a=tag:item do=add:picked:1
",
    );
    let mut world = World::new(&def, 1);
    let lamp = world.entities()[0].id;
    let inputs = Inputs {
        host: vec![
            HostEvent {
                name: "door_opened".to_owned(),
                a: None,
                b: None,
            },
            HostEvent {
                name: "grabbed".to_owned(),
                a: Some(lamp),
                b: None,
            },
        ],
        ..Inputs::default()
    };
    world.step(&inputs).unwrap();
    assert_eq!((world.vars()["opened"], world.vars()["picked"]), (1.0, 1.0));
    world.step(&Inputs::default()).unwrap();
    assert_eq!(
        (world.vars()["opened"], world.vars()["picked"]),
        (1.0, 1.0),
        "events last one step"
    );
}

#[test]
fn the_same_definition_seed_and_inputs_give_the_same_hash_and_state_restores_exactly() {
    let def = parse(COINS);
    let run = |seed: u64| {
        let mut world = World::new(&def, seed);
        for _ in 0..600 {
            let inputs = steer(&world, "player", "coin");
            world.step(&inputs).unwrap();
        }
        world.hash()
    };
    assert_eq!(run(7), run(7));
    assert_ne!(
        run(7),
        run(8),
        "a different seed scatters the coins differently"
    );

    let mut world = World::new(&def, 7);
    for _ in 0..100 {
        let inputs = steer(&world, "player", "coin");
        world.step(&inputs).unwrap();
    }
    let saved = world.snapshot();
    let play = |world: &mut World| {
        for _ in 0..100 {
            let inputs = steer(world, "player", "coin");
            world.step(&inputs).unwrap();
        }
        world.hash()
    };
    let first = play(&mut world);
    world.restore(&saved).unwrap();
    assert_eq!(play(&mut world), first, "snapshot, run, restore, run again");
    let other = parse("game name=other\nkind a size=1 speed=1 tags=x\nspawn a at=0,0,0");
    let mut stranger = World::new(&other, 1);
    assert_eq!(stranger.restore(&saved), Err(GameError::SnapshotMismatch));
}

#[test]
fn budgets_are_enforced_with_typed_errors_and_never_hang() {
    let crowded = parse(
        "game name=crowd\nkind dot size=0.1 tags=dot\nspawn dot at=0,0,0 count=3000 spread=50",
    );
    assert!(
        crowded
            .validate()
            .iter()
            .any(|d| d.message.contains("2048")),
        "validate names the entity budget"
    );
    let mut world = World::new(&crowded, 1);
    assert!(matches!(
        world.step(&Inputs::default()),
        Err(GameError::EntityBudget(_))
    ));
    let looping = parse(
        "game name=loop\nkind a size=1 tags=a\nspawn a at=0,0,0\nrule on=start do=emit:ping\nrule on=emit:ping do=emit:ping",
    );
    let mut world = World::new(&looping, 1);
    assert_eq!(
        world.step(&Inputs::default()),
        Err(GameError::ChainDepth(8)),
        "an event that raises itself stops at depth 8"
    );
    let nan = Inputs {
        axes: BTreeMap::from([("move.x".to_owned(), f64::NAN)]),
        ..Inputs::default()
    };
    let def = parse(COINS);
    let mut world = World::new(&def, 1);
    assert!(matches!(world.step(&nan), Err(GameError::InvalidInput(_))));
}

#[test]
fn ten_malformed_lines_each_give_a_located_diagnostic_with_a_suggestion() {
    let cases: [(&str, &str); 10] = [
        ("kind", "a kind needs a name"),
        ("spawn ghost at=0,0,0", "unknown kind"),
        ("rule on=tick do", "missing ="),
        ("rule on=tick when=score>>1 do=win", "bad expression"),
        ("goal win", "a goal needs when="),
        ("var score", "missing ="),
        ("rule on=bogus do=win", "unknown event"),
        (
            "rule on=tick do=add:score",
            "add needs a variable and an amount",
        ),
        ("rule on=tick do=destroy:nobody", "bad selector"),
        ("kind a size=big", "not a number"),
    ];
    for (line, what) in cases {
        let text = format!("game name=t\n{line}\n");
        let Err(found) = GameDef::parse(&text) else {
            panic!("`{line}` ({what}) should be refused")
        };
        let first = &found[0];
        assert_eq!(
            first.line, 2,
            "`{line}` ({what}) is reported on line 2: {first:?}"
        );
        assert!(first.column >= 1, "`{line}` has a column");
        assert!(
            !first.message.is_empty() && !first.suggestion.is_empty(),
            "`{line}` explains itself and says how to fix it: {first:?}"
        );
    }
}

#[test]
fn validation_finds_rules_that_never_fire_and_goals_that_cannot_be_reached() {
    assert!(
        parse(COINS).validate().is_empty(),
        "the coin game is sound: {:?}",
        parse(COINS).validate()
    );
    let ghosts = parse(
        "game name=ghosts
var score=0
kind hero size=0.5 tags=hero
spawn hero at=0,0,0
rule on=touch a=tag:hero b=tag:ghost do=add:score:1
goal win when=score>=3
",
    );
    let problems = ghosts.validate();
    assert!(
        problems.iter().any(|d| d.message.contains("ghost")),
        "the rule can never fire: {problems:?}"
    );
    let dead = parse(
        "game name=dead\nvar score=0\nkind hero size=0.5 tags=hero\nspawn hero at=0,0,0\ngoal win when=score>=3",
    );
    assert!(
        dead.validate().iter().any(|d| d.message.contains("score")),
        "nothing ever changes score"
    );
}

#[test]
fn describe_is_a_compact_summary_a_model_can_read() {
    let text = parse(COINS).describe();
    for word in [
        "coins", "player", "coin", "spike", "score", "lives", "time", "goal",
    ] {
        assert!(text.contains(word), "the summary mentions {word}: {text}");
    }
    assert!(text.len() < 2000, "compact: {} bytes", text.len());
}
