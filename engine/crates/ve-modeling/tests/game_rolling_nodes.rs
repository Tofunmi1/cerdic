//! The game nodes for rolling things and hand-held tools (ADR 0068): `game.terrain`, `game.surface`, and the `roll`, `swing`, `reach`, `loft`, `power` and `pocket` parameters of
//! `game.kind` write the matching game text lines, so a model builds a course as nodes. Written before the bodies; do not weaken.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup unwraps data that is known to be valid"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn game_text(source: &str) -> String {
    let commands = parse_text(&builtin_registry(), source).unwrap().commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_golf_nodes").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    let evaluation = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap();
    evaluation
        .game
        .expect("the nodes collect into a game")
        .describe()
}

const NODES: &str = "\
hero = game.kind@1(name=\"player\", size=0.4, tags=[\"hero\"])
ball = game.kind@1(name=\"ball\", size=0.03, roll=true, bounce=0.5)
putter = game.kind@1(name=\"putter\", size=0.05, tags=[\"hand_r\"], swing=true, reach=0.9, loft=0, power=1.2, part=\"putter_mesh\")
wedge = game.kind@1(name=\"wedge\", size=0.05, tags=[\"hand_l\"], swing=true, reach=1.0, loft=50)
cup = game.kind@1(name=\"cup\", size=0.07, pocket=true)
ground = game.terrain@1(parts=[\"course\", \"green\"], cell=0.2)
sand = game.surface@1(name=\"sand\", parts=[\"bunker\"], decel=8, bounce=0.1)
fast = game.surface@1(name=\"green\", parts=[\"green\"], decel=0.55, bounce=0.5)
all = game.collect@1(hero, ball, putter, wedge, cup, ground, sand, fast)
output rules = all
";

#[test]
fn the_nodes_write_the_rolling_lines_and_keys() {
    let text = game_text(NODES);
    assert!(
        text.lines().any(|l| l.starts_with("kind ball")
            && l.contains("roll=1")
            && l.contains("bounce=0.5")),
        "{text}"
    );
    assert!(
        text.lines().any(|l| l.starts_with("kind putter")
            && l.contains("swing=1")
            && l.contains("reach=0.9")
            && l.contains("power=1.2")),
        "{text}"
    );
    assert!(
        text.lines()
            .any(|l| l.starts_with("kind wedge") && l.contains("loft=50") && l.contains("reach=1")),
        "{text}"
    );
    assert!(
        text.lines()
            .any(|l| l.starts_with("kind cup") && l.contains("pocket=1")),
        "{text}"
    );
    assert!(
        text.contains("terrain parts=course,green cell=0.2"),
        "{text}"
    );
    assert!(
        text.contains("surface sand parts=bunker decel=8 bounce=0.1"),
        "{text}"
    );
    assert!(
        text.contains("surface green parts=green decel=0.55 bounce=0.5"),
        "{text}"
    );
}

#[test]
fn the_surfaces_keep_the_order_they_were_collected_in() {
    // Their ids are 1, 2, ... in text order, so a model can rely on later surfaces winning where parts overlap.
    let text = game_text(NODES);
    let sand = text.find("surface sand").unwrap();
    let green = text.find("surface green").unwrap();
    assert!(sand < green, "{text}");
}

#[test]
fn the_text_the_nodes_make_parses_back_to_the_same_game() {
    let text = game_text(NODES);
    assert_eq!(text, ve_game::GameDef::parse(&text).unwrap().describe());
}

#[test]
fn the_new_nodes_are_registered_and_ready() {
    let registry = builtin_registry();
    let ready: Vec<String> = registry.ready().map(|spec| spec.id.to_string()).collect();
    for id in ["game.terrain@1", "game.surface@1"] {
        assert!(ready.iter().any(|r| r == id), "{id} is a ready operator");
    }
}

#[test]
fn a_surface_without_parts_or_with_a_bad_decel_is_a_problem_naming_it() {
    for source in [
        "s = game.surface@1(name=\"mud\", parts=[], decel=3)\nall = game.collect@1(s)\noutput rules = all\n",
        "s = game.surface@1(name=\"mud\", parts=[\"p\"], decel=0)\nall = game.collect@1(s)\noutput rules = all\n",
    ] {
        let commands = parse_text(&builtin_registry(), source).unwrap().commands;
        let mut session = Session::with_builtin(9);
        let applied = session.apply(&CommandBatch {
            doc: session.document().id.clone(),
            base_revision: 0,
            transaction: TransactionId::parse("t_golf_bad").unwrap(),
            label: None,
            commands,
        });
        let message = match applied {
            Outcome::Committed { .. } => {
                match Evaluator::default().evaluate(session.document(), Quality::Final) {
                    Ok(evaluation) => format!("{:?}", evaluation.game.map(|g| g.describe())),
                    Err(error) => format!("{error:?}"),
                }
            }
            other => format!("{other:?}"),
        };
        assert!(
            message.contains("mud") || message.contains("decel") || message.contains("parts"),
            "{message}"
        );
        assert!(
            !message.contains("surface mud parts=")
                || message.contains("rror")
                || message.contains("iagnostic"),
            "{message}"
        );
    }
}
