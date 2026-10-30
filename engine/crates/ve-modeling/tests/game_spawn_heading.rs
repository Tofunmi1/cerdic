//! `game.spawn(heading=DEGREES)` carries the way an entity first faces into the game text (ADR 0066). Written before the body; do not weaken.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup unwraps data that is known to be valid"
)]
use ve_game::World;
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn game_of(source: &str) -> ve_game::GameDef {
    let commands = parse_text(&builtin_registry(), source).unwrap().commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_spawn_heading").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    let evaluation = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap();
    evaluation.game.expect("the nodes collect into a game")
}

const NODES: &str = "\
guard = game.kind@1(name=\"guard\", size=0.5)
a = game.spawn@1(kind=\"guard\", at=[0, 0, 0], heading=90)
b = game.spawn@1(kind=\"guard\", at=[2, 0, 0])
all = game.collect@1(guard, a, b)
output rules = all
";

#[test]
fn a_spawn_node_can_give_the_way_it_faces() {
    let world = World::new(&game_of(NODES), 1);
    let headings: Vec<f64> = world.entities().iter().map(|e| e.heading).collect();
    assert_eq!(headings.len(), 2);
    assert!(
        (headings[0] - std::f64::consts::FRAC_PI_2).abs() < 1e-9,
        "{headings:?}"
    );
    assert_eq!(
        headings[1].to_bits(),
        0.0_f64.to_bits(),
        "a spawn without one faces +Z"
    );
}
