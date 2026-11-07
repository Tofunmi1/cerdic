//! `poly.round_edges` on a doorway cut down to the floor: the corner where three rounded edges meet folds at more segments, so the node
//! rounds with fewer and says so in a warning instead of failing or quietly doing less.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let commands = parse_text(&builtin_registry(), source).map_err(|e| format!("{e:?}"))?.commands;
    let mut session = Session::with_builtin(21);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_round_reduced").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default().evaluate(session.document(), Quality::Final).map_err(|e| format!("{e:?}"))
}

const DOORWAY: &str = "body = solid.box@1(size=[6, 2.8, 0.5])\ncut = solid.box@1(size=[1.2, 2, 1])\ncut_at = xform.move@1(cut, by=[0, -0.4, 0])\nwall = solid.difference@1(body, cut_at)\np = poly.from_solid@1(wall)\n";

#[test]
fn a_fold_at_the_requested_segments_gives_fewer_segments_and_a_warning() {
    let wide = evaluate(&format!("{DOORWAY}r = poly.round_edges@1(p, width=0.03, segments=3)\nso = poly.to_solid@1(r)\noutput s = so\n")).unwrap();
    let reduced = wide.warnings.iter().find(|w| w.code == "poly.round_reduced");
    let message = reduced.map(|w| w.message.as_str()).unwrap_or_default();
    assert!(message.contains("instead of 3"), "the warning says what was asked and what was done: {:?}", wide.warnings);
}

#[test]
fn a_mesh_that_rounds_at_the_requested_segments_gives_no_warning() {
    let ev = evaluate("b = solid.box@1(size=[1, 1, 1])\np = poly.from_solid@1(b)\nr = poly.round_edges@1(p, width=0.05, segments=3)\nso = poly.to_solid@1(r)\noutput s = so\n").unwrap();
    assert!(!ev.warnings.iter().any(|w| w.code == "poly.round_reduced"), "{:?}", ev.warnings);
}
