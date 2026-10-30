//! The game as nodes (ADR 0062 phase 2): `game.*` operators make fragments of a rules definition, `game.collect` joins them into the
//! one `GameDef` that `Evaluation::game` carries. Written before the bodies; do not weaken.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup unwraps data that is known to be valid"
)]
use std::collections::BTreeMap;
use ve_game::{GameDef, Inputs, Status, World};
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| format!("{e:?}"))?
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g14_game").unwrap(),
        label: None,
        commands,
    });
    if !matches!(applied, Outcome::Committed { .. }) {
        return Err(format!("{applied:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| format!("{e:?}"))
}

/// The coin game of `ve-game`'s own tests, one node per line.
const COIN_NODES: &str = "\
score = game.var@1(name=\"score\", value=0)
lives = game.var@1(name=\"lives\", value=3)
clock = game.var@1(name=\"time\", value=0)
hero = game.kind@1(name=\"player\", size=0.5, speed=4, tags=[\"hero\"])
coin = game.kind@1(name=\"coin\", size=0.4, tags=[\"pickup\"])
spike = game.kind@1(name=\"spike\", size=0.5, tags=[\"hazard\"])
p0 = game.spawn@1(kind=\"player\", at=[0, 0, 0])
c0 = game.spawn@1(kind=\"coin\", at=[3, 0, 2], count=8, spread=6)
move = game.input@1(name=\"move\", axis=\"x,z\", target=\"tag:hero\")
r_pick = game.rule@1(on=\"touch\", a=\"tag:hero\", b=\"tag:pickup\", do=\"add:score:1;destroy:b;effect:sfx:pickup@b\")
r_hurt = game.rule@1(on=\"touch\", a=\"tag:hero\", b=\"tag:hazard\", do=\"add:lives:-1;destroy:b\")
r_tick = game.rule@1(on=\"tick\", every=1.0, do=\"add:time:1\")
r_dead = game.rule@1(on=\"var:lives\", when=\"lives<=0\", do=\"lose\")
g_win = game.goal@1(outcome=\"win\", when=\"score>=8\")
g_lose = game.goal@1(outcome=\"lose\", when=\"time>=60\")
all = game.collect@1(score, lives, clock, hero, coin, spike, p0, c0, move, r_pick, r_hurt, r_tick, r_dead, g_win, g_lose, name=\"coins\")
output rules = all
";

const COIN_TEXT: &str = "game name=coins
var score=0 lives=3 time=0
kind player size=0.5 speed=4 tags=hero
kind coin size=0.4 tags=pickup
kind spike size=0.5 tags=hazard
spawn player at=0,0,0
spawn coin at=3,0,2 count=8 spread=6
input move axis=x,z target=tag:hero
rule on=touch a=tag:hero b=tag:pickup do=add:score:1;destroy:b;effect:sfx:pickup@b
rule on=touch a=tag:hero b=tag:hazard do=add:lives:-1;destroy:b
rule on=tick every=1.0 do=add:time:1
rule on=var:lives when=lives<=0 do=lose
goal win when=score>=8
goal lose when=time>=60
";

#[test]
fn the_game_operators_are_registered_and_ready() {
    let registry = builtin_registry();
    let ready: Vec<String> = registry.ready().map(|spec| spec.id.to_string()).collect();
    for id in [
        "game.var@1",
        "game.kind@1",
        "game.spawn@1",
        "game.input@1",
        "game.rule@1",
        "game.goal@1",
        "game.collect@1",
    ] {
        assert!(
            ready.iter().any(|r| r == id),
            "{id} is a ready operator: {ready:?}"
        );
    }
}

#[test]
fn nodes_collect_into_the_definition_the_text_form_would_give() {
    let result = evaluate(COIN_NODES).unwrap();
    let def = result
        .game
        .as_ref()
        .expect("a collected game is carried by the evaluation");
    let expected = GameDef::parse(COIN_TEXT).unwrap();
    // The text form keeps variables in name order, nodes keep the order they were linked in; everything else is compared as printed.
    let without_vars = |d: &GameDef| {
        d.describe()
            .lines()
            .filter(|l| !l.starts_with("vars:"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(
        without_vars(def),
        without_vars(&expected),
        "same game, same canonical text"
    );
    let names = |d: &GameDef| {
        let mut n: Vec<_> = d
            .vars
            .iter()
            .map(|v| (v.name.clone(), v.value.to_bits()))
            .collect();
        n.sort();
        n
    };
    assert_eq!(
        names(def),
        names(&expected),
        "same variables with the same starting values"
    );
    assert_eq!(
        (
            def.vars.len(),
            def.kinds.len(),
            def.rules.len(),
            def.goals.len()
        ),
        (3, 3, 4, 2)
    );
    assert!(result.parts.is_empty(), "a game is not geometry");
}

#[test]
fn a_collected_game_is_playable() {
    let def = evaluate(COIN_NODES).unwrap().game.unwrap();
    let mut world = World::new(&def, 7);
    let mut status = Status::Playing;
    for _ in 0..3_000 {
        let hero = world
            .entities()
            .iter()
            .find(|e| e.kind == "player")
            .map(|e| e.position);
        let coin = world
            .entities()
            .iter()
            .filter(|e| e.kind == "coin")
            .map(|e| e.position)
            .min_by(|a, b| {
                let d = |p: &[f64; 3]| {
                    hero.map_or(0.0, |h| (p[0] - h[0]).powi(2) + (p[2] - h[2]).powi(2))
                };
                d(a).total_cmp(&d(b))
            });
        let inputs = match (hero, coin) {
            (Some(h), Some(c)) => {
                let (dx, dz) = (c[0] - h[0], c[2] - h[2]);
                let len = (dx * dx + dz * dz).sqrt().max(1e-9);
                Inputs {
                    axes: BTreeMap::from([
                        ("move.x".to_owned(), dx / len),
                        ("move.z".to_owned(), dz / len),
                    ]),
                    ..Inputs::default()
                }
            }
            _ => Inputs::default(),
        };
        status = world.step(&inputs).unwrap().status;
        if status != Status::Playing {
            break;
        }
    }
    assert_eq!(
        status,
        Status::Won,
        "a bot that walks to the nearest coin wins the game the nodes describe"
    );
}

#[test]
fn a_scene_without_game_nodes_carries_no_game() {
    let result = evaluate("b = solid.box@1(size=[1, 1, 1])\noutput b = b\n").unwrap();
    assert!(result.game.is_none());
}

#[test]
fn kits_join_into_one_game() {
    let source = "\
s = game.var@1(name=\"score\", value=0)
coin = game.kind@1(name=\"coin\", tags=[\"pickup\"])
hero = game.kind@1(name=\"player\", tags=[\"hero\"])
pick = game.rule@1(on=\"touch\", a=\"tag:hero\", b=\"tag:pickup\", do=\"add:score:1;destroy:b\")
win = game.goal@1(outcome=\"win\", when=\"score>=1\")
kit_a = game.collect@1(s, coin, pick)
kit_b = game.collect@1(hero, win)
both = game.collect@1(kit_a, kit_b, name=\"joined\")
output rules = both
";
    let def = evaluate(source).unwrap().game.unwrap();
    assert_eq!(
        (
            def.vars.len(),
            def.kinds.len(),
            def.rules.len(),
            def.goals.len()
        ),
        (1, 2, 1, 1)
    );
    assert_eq!(def.name, "joined");
}

#[test]
fn a_bad_rule_is_refused_at_the_node_that_wrote_it() {
    let source = COIN_NODES.replace("do=\"add:time:1\"", "do=\"explode:time\"");
    let error = evaluate(&source).unwrap_err();
    assert!(error.contains("game."), "a game diagnostic code: {error}");
    assert!(
        error.contains("r_tick"),
        "names the node whose rule is wrong, not a line of the internal text: {error}"
    );
}

#[test]
fn a_duplicate_name_is_refused_with_both_nodes_named() {
    let source = "a = game.var@1(name=\"score\", value=0)\nb = game.var@1(name=\"score\", value=5)\nall = game.collect@1(a, b)\noutput rules = all\n";
    let error = evaluate(source).unwrap_err();
    assert!(error.contains("game.duplicate"), "{error}");
    assert!(error.contains('a') && error.contains('b'), "{error}");
}

#[test]
fn the_budgets_of_the_rules_core_hold_at_estimate_time() {
    let mut source = String::new();
    // A collect takes at most 64 links, so 65 kinds arrive through two kits.
    let mut halves = [Vec::new(), Vec::new()];
    for i in 0..65 {
        source.push_str(&format!("k{i} = game.kind@1(name=\"kind{i}\")\n"));
        halves[usize::from(i >= 33)].push(format!("k{i}"));
    }
    source.push_str(&format!("a = game.collect@1({})\nb = game.collect@1({})\nall = game.collect@1(a, b)\noutput rules = all\n", halves[0].join(", "), halves[1].join(", ")));
    let error = evaluate(&source).unwrap_err();
    assert!(
        error.contains("budget."),
        "more than 64 kinds is a budget refusal: {error}"
    );
}

#[test]
fn a_game_fragment_is_not_geometry_and_says_where_to_link_it() {
    let source = "v = game.var@1(name=\"score\", value=0)\nb = solid.box@1(size=[1, 1, 1])\nx = xform.move@1(v, by=[0, 1, 0])\noutput x = x\n";
    let error = evaluate(source).unwrap_err();
    assert!(error.contains("graph.port_type"), "{error}");
}

const GROUND_AND_COINS: &str = "t = terrain.tile@1(size=[128, 128], cell=2, seed=4, height=60, scale=160)\nb = solid.box@1(size=[1, 1, 1])\ns = scatter.place@1(b, t, radius=16, seed=2)\n";

#[test]
fn a_spawn_from_a_placed_set_makes_one_entity_per_placement_standing_where_it_was_placed() {
    let planted: Vec<[f32; 3]> = {
        let result = evaluate(&format!("{GROUND_AND_COINS}output o = s\n")).unwrap();
        result.parts[0]
            .transforms
            .as_ref()
            .unwrap()
            .iter()
            .map(|m| [m[12], m[13], m[14]])
            .collect()
    };
    assert!(
        planted.len() > 10,
        "the fixture places a good number: {}",
        planted.len()
    );
    let source = format!(
        "{GROUND_AND_COINS}coin = game.kind@1(name=\"coin\", tags=[\"pickup\"])\nsp = game.spawn@1(s, kind=\"coin\")\nall = game.collect@1(coin, sp)\noutput rules = all\n"
    );
    let def = evaluate(&source).unwrap().game.unwrap();
    assert_eq!(def.spawns.len(), planted.len(), "one spawn per placement");
    for (spawn, place) in def.spawns.iter().zip(&planted) {
        for axis in 0..3 {
            assert!(
                (spawn.position[axis] - f64::from(place[axis])).abs() < 1e-3,
                "{:?} against {place:?}",
                spawn.position
            );
        }
        assert_eq!(spawn.count, 1);
    }
    let (low, high) = planted
        .iter()
        .fold((f32::MAX, f32::MIN), |(l, h), p| (l.min(p[1]), h.max(p[1])));
    assert!(
        high - low > 1.0,
        "the ground is hilly, so the coins stand at different heights: {low} to {high}"
    );
}

#[test]
fn hud_nodes_show_variables_and_name_the_node_when_one_is_missing() {
    let source = "\
score = game.var@1(name=\"score\", value=0)
hero = game.kind@1(name=\"player\", tags=[\"hero\"])
p0 = game.spawn@1(kind=\"player\", at=[0, 0, 0])
panel = game.hud@1(text=\"Score {score}\", anchor=\"gaze\", size=0.04)
all = game.collect@1(score, hero, p0, panel)
output rules = all
";
    let def = evaluate(source).unwrap().game.unwrap();
    assert_eq!(def.huds.len(), 1);
    assert_eq!(def.huds[0].text, "Score {score}");
    assert!((def.huds[0].size - 0.04).abs() < 1e-9);
    let broken = evaluate(&source.replace("{score}", "{scroe}")).unwrap_err();
    assert!(
        broken.contains("game.hud") && broken.contains("panel") && broken.contains("scroe"),
        "{broken}"
    );
}

#[test]
fn a_kind_can_be_a_solid_box_the_hero_cannot_enter() {
    let source = "\
hero = game.kind@1(name=\"player\", tags=[\"hero\"])
door = game.kind@1(name=\"door\", tags=[\"gate\"], solid=[0.5, 2.6, 7.6])
wall = game.kind@1(name=\"coin\", tags=[\"pickup\"])
all = game.collect@1(hero, door, wall)
output rules = all
";
    let def = evaluate(source).unwrap().game.unwrap();
    let solid = |name: &str| def.kinds.iter().find(|k| k.name == name).unwrap().solid;
    assert_eq!(solid("door"), Some([0.5, 2.6, 7.6]));
    assert_eq!(solid("coin"), None, "no solid parameter, no obstacle");
}

#[test]
fn too_many_placements_are_a_budget_refusal() {
    let source = "t = terrain.tile@1(size=[256, 256], cell=2, seed=4, height=10, scale=160)\nb = solid.box@1(size=[0.1, 0.1, 0.1])\ns = scatter.place@1(b, t, radius=2, seed=2)\ncoin = game.kind@1(name=\"coin\")\nsp = game.spawn@1(s, kind=\"coin\")\nall = game.collect@1(coin, sp)\noutput rules = all\n";
    let error = evaluate(source).unwrap_err();
    assert!(
        error.contains("budget."),
        "more than 2,048 entities is refused: {error}"
    );
}

#[test]
fn the_same_nodes_give_the_same_definition_every_time() {
    let a = evaluate(COIN_NODES).unwrap().game.unwrap().describe();
    let b = evaluate(COIN_NODES).unwrap().game.unwrap().describe();
    assert_eq!(a, b);
}

#[test]
fn player_kind_binds_a_scene_part_and_follows_the_viewer_through_evaluation() {
    let source = "\
hero = game.kind@1(name=\"player\", size=0.5, tags=[\"hero\"], part=\"body\")
coin = game.kind@1(name=\"coin\", size=0.4, tags=[\"pickup\"], part=\"coin_mesh\")
p0 = game.spawn@1(kind=\"player\", at=[0, 0, 0])
c0 = game.spawn@1(kind=\"coin\", at=[4, 0, 0])
move = game.input@1(name=\"body\", follow=\"viewer\", target=\"tag:hero\")
pick = game.rule@1(on=\"touch\", a=\"tag:hero\", b=\"tag:pickup\", do=\"add:score:1;destroy:b\")
score = game.var@1(name=\"score\", value=0)
won = game.goal@1(outcome=\"win\", when=\"score>=1\")
all = game.collect@1(score, hero, coin, p0, c0, move, pick, won)
output rules = all
body = solid.box@1(size=[1, 1, 1])
output body = body
";
    let def = evaluate(source).unwrap().game.unwrap();
    assert_eq!(def.kinds[0].part.as_deref(), Some("body"));
    assert_eq!(def.kinds[1].part.as_deref(), Some("coin_mesh"));
    let mut world = World::new(&def, 1);
    let outcome = world
        .step(&Inputs {
            viewer: Some([4.0, 0.0, 0.0]),
            ..Inputs::default()
        })
        .unwrap();
    assert_eq!(outcome.status, Status::Won);
}
