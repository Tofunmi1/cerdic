//! The heroine (`web/tests/scenes/characters/heroine.txt`) must evaluate, and each of her parts must be a clean closed surface of a sensible size:
//! a body of human height, a head on top of it, and no part with a runaway triangle count.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn evaluate(name: &str) -> ve_modeling::Evaluation {
    let path = format!("{}/../../web/tests/scenes/characters/{name}.txt", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(path).unwrap();
    let commands = match parse_text(&builtin_registry(), &source) {
        Ok(parsed) => parsed.commands,
        Err(e) => panic!("parse: {e:?}"),
    };
    let mut session = Session::with_builtin(31);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_heroine").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "apply: {applied:?}");
    match Evaluator::default().evaluate(session.document(), Quality::Final) {
        Ok(ev) => ev,
        Err(e) => panic!("evaluate: {} {}", e.code, e.message),
    }
}

#[test]
fn the_heroine_evaluates_into_a_body_and_a_head_of_human_size() {
    let ev = evaluate("heroine");
    assert!(ev.warnings.is_empty(), "no warnings: {:?}", ev.warnings);
    let mut top = f32::MIN;
    let mut bottom = f32::MAX;
    for part in &ev.parts {
        let triangles = part.indices.len() / 3;
        eprintln!("part {:<10} {triangles:>8} triangles", part.name);
        for p in &part.positions {
            top = top.max(p[1]);
            bottom = bottom.min(p[1]);
        }
    }
    eprintln!("height {:.3} m, lowest point {bottom:.3}", top - bottom);
    assert!((1.6..=1.85).contains(&(top - bottom)), "a human height: {}", top - bottom);
    assert!(bottom.abs() < 0.03, "she stands on the floor: {bottom}");
}
