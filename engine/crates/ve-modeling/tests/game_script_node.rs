//! `game.script` makes a JavaScript behaviour a node of the game: it joins the collected definition and runs in the fixed step.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup unwraps data that is known to be valid"
)]
use ve_game::{Inputs, World};
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

const NODES: &str = "\
score = game.var@1(name=\"score\", value=0)
hero = game.kind@1(name=\"player\", size=0.5, speed=4, tags=[\"hero\"])
coin = game.kind@1(name=\"coin\", size=0.4, tags=[\"pickup\"])
p0 = game.spawn@1(kind=\"player\", at=[0, 0, 0])
c0 = game.spawn@1(kind=\"coin\", at=[0.2, 0, 0])
pick = game.script@1(name=\"pickups\", source=\"on('touch', {a: 'tag:hero', b: 'tag:pickup'}, (e) => { game.add('score', 1); e.b.destroy(); });\")
all = game.collect@1(score, hero, coin, p0, c0, pick, name=\"scripted\")
output rules = all
";

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| format!("{e:?}"))?
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_script").unwrap(),
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

#[test]
fn a_script_node_joins_the_game_and_runs() {
    let evaluation = evaluate(NODES).unwrap();
    let definition = evaluation.game.clone().unwrap();
    assert_eq!(definition.scripts.len(), 1);
    assert_eq!(definition.scripts[0].name, "pickups");
    let mut world = World::new(&definition, 1);
    for _ in 0..3 {
        world.step(&Inputs::default()).unwrap();
    }
    assert_eq!(world.vars()["score"], 1.0);
    assert!(world.take_script_log().is_empty());
}

#[test]
fn a_blank_script_is_refused_naming_the_node() {
    let blank = NODES.replace(
        "source=\"on('touch', {a: 'tag:hero', b: 'tag:pickup'}, (e) => { game.add('score', 1); e.b.destroy(); });\"",
        "source=\"   \"",
    );
    let error = evaluate(&blank).unwrap_err();
    assert!(
        error.contains("game.script") && error.contains("needs source"),
        "{error}"
    );
}
