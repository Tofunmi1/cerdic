//! The game nodes for creatures, throwing and the walkable level (ADR 0067): `game.agent`, `game.nav`, `game.ground`, and the `fall` and
//! `bounce` parameters of `game.kind` write the matching game text lines, so a model builds creatures and throwable things as nodes.
//! Written before the bodies; do not weaken.
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
        transaction: TransactionId::parse("t_agent_nodes").unwrap(),
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
hero = game.kind@1(name=\"player\", size=0.5, tags=[\"hero\"])
bot = game.kind@1(name=\"creature\", size=0.4, tags=[\"creature\"], part=\"robot\")
rock = game.kind@1(name=\"rock\", size=0.15, tags=[\"item\"], grab=true, fall=true, bounce=0.5)
ground = game.ground@1(y=0.25)
nav = game.nav@1(parts=[\"floor\", \"stairs\"], cell=0.2, radius=0.35)
watch = game.agent@1(kind=\"creature\", patrol=[[-6, -6], [6, -6], [6, 6]], speed=1.5, chase_speed=3, sight=5, lose=9)
all = game.collect@1(hero, bot, rock, ground, nav, watch)
output rules = all
";

#[test]
fn the_nodes_write_the_creature_throwing_and_nav_lines() {
    let text = game_text(NODES);
    assert!(
        text.contains("agent creature") && text.contains("patrol=-6,-6;6,-6;6,6"),
        "{text}"
    );
    assert!(
        text.contains("speed=1.5")
            && text.contains("chase_speed=3")
            && text.contains("sight=5")
            && text.contains("lose=9"),
        "{text}"
    );
    assert!(
        text.contains("nav parts=floor,stairs")
            && text.contains("cell=0.2")
            && text.contains("radius=0.35"),
        "{text}"
    );
    assert!(text.contains("ground y=0.25"), "{text}");
    assert!(
        text.lines().any(|l| l.starts_with("kind rock")
            && l.contains("fall=1")
            && l.contains("bounce=0.5")),
        "{text}"
    );
}

#[test]
fn the_text_the_nodes_make_parses_back_to_the_same_game() {
    let text = game_text(NODES);
    let again = ve_game::GameDef::parse(&text).unwrap().describe();
    assert_eq!(text, again);
}

#[test]
fn the_new_nodes_are_registered_and_ready() {
    let registry = builtin_registry();
    let ready: Vec<String> = registry.ready().map(|spec| spec.id.to_string()).collect();
    for id in ["game.agent@1", "game.nav@1", "game.ground@1"] {
        assert!(ready.iter().any(|r| r == id), "{id} is a ready operator");
    }
}

#[test]
fn an_agent_for_a_kind_that_does_not_exist_is_a_problem_naming_it() {
    let source = "bot = game.kind@1(name=\"creature\", size=0.4)\nwatch = game.agent@1(kind=\"ghost\", patrol=[[0, 0], [1, 1]], speed=1)\nall = game.collect@1(bot, watch)\noutput rules = all\n";
    let commands = parse_text(&builtin_registry(), source).unwrap().commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_agent_ghost").unwrap(),
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
    assert!(message.contains("ghost"), "{message}");
}
