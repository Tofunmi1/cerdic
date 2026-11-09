//! The harbour scene (`web/tests/scenes/harbour/quay.txt`) must parse, apply and evaluate, and every feature it names must show up in the result.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

#[test]
fn the_quay_scene_evaluates() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../web/tests/scenes/harbour/quay.txt");
    let source = std::fs::read_to_string(path).unwrap();
    let commands = match parse_text(&builtin_registry(), &source) {
        Ok(parsed) => parsed.commands,
        Err(e) => panic!("parse: {e:?}"),
    };
    let mut session = Session::with_builtin(77);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_quay").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "apply: {applied:?}");
    let ev = match Evaluator::default().evaluate(session.document(), Quality::Final) {
        Ok(ev) => ev,
        Err(e) => panic!("evaluate: {} {}", e.code, e.message),
    };
    assert!(ev.parts.len() >= 50, "the scene has about fifty parts, got {}", ev.parts.len());
    assert!(ev.warnings.is_empty(), "no warnings: {:?}", ev.warnings);
}
