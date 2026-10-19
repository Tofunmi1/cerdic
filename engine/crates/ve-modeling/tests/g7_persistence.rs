//! G7 persistence and instancing through a subdivided graph.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "the persistence fixture fails immediately when its fixed graph is invalid"
)]
use ve_graph::{
    CommandBatch, Document, Outcome, Session, TransactionId, builtin_registry, parse_text,
};
use ve_modeling::{Evaluator, Quality};

const PROGRAM: &str = "group subdivided_block\n\
    box = solid.box@1(size=[1, 1, 1])\n\
    cage = poly.from_solid@1(box)\n\
    smooth = poly.subdivide@1(cage, levels=1)\n\
    solid = poly.to_solid@1(smooth)\n\
    expose size = box.size\n\
    result solid\n\
    end\n\
    left = subdivided_block()\n\
    right = subdivided_block()\n\
    output left\n\
    output right";

fn session() -> Session {
    let commands = parse_text(&builtin_registry(), PROGRAM)
        .unwrap_or_else(|errors| panic!("parse errors: {errors:?}"))
        .commands;
    let mut session = Session::with_builtin(14);
    let result = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g7_group_init").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(result, Outcome::Committed { .. }), "{result:?}");
    session
}

fn evaluate(session: &Session) -> ve_modeling::Evaluation {
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap_or_else(|error| panic!("{}: {}", error.code, error.message))
}

#[test]
fn subdivided_group_instances_survive_reload_and_parameter_history() {
    let mut session = session();
    let initial = evaluate(&session);
    assert_eq!(initial.parts.len(), 2);
    assert!(initial.parts.iter().all(|part| part.indices.len() == 216));
    assert!((initial.parts[0].volume - initial.parts[1].volume).abs() < 1.0e-12);

    let saved = session.document().canonical_json();
    let document: Document = serde_json::from_str(&saved).unwrap();
    assert_eq!(document.canonical_json(), saved);
    let reloaded = evaluate(&Session::new(builtin_registry(), document));
    for (before, after) in initial.parts.iter().zip(&reloaded.parts) {
        assert_eq!(before.positions, after.positions);
        assert_eq!(before.indices, after.indices);
    }

    let size_node = session
        .document()
        .nodes
        .iter()
        .find(|node| node.label.as_deref() == Some("box") && node.group.is_some())
        .expect("the grouped cage has a source box")
        .id
        .clone();
    let edit = serde_json::json!({
        "doc": session.document().id,
        "base_revision": session.document().revision,
        "transaction": "t_g7_group_size",
        "commands": [{"set_param": {"node": size_node, "param": "size", "value": [2, 1, 1]}}]
    });
    assert!(matches!(
        session.apply_json(&edit.to_string()),
        Outcome::Committed { .. }
    ));
    let edited = evaluate(&session);
    assert!(
        edited.parts[0].volume > initial.parts[0].volume,
        "{initial:?} -> {edited:?}"
    );
    assert!(
        edited.parts[1].volume > initial.parts[1].volume,
        "{initial:?} -> {edited:?}"
    );

    assert!(session.undo().is_some());
    let undone = evaluate(&session);
    assert!((undone.parts[0].volume - initial.parts[0].volume).abs() < 1.0e-12);
    assert!(session.redo().is_some());
    let redone = evaluate(&session);
    assert!((redone.parts[1].volume - edited.parts[1].volume).abs() < 1.0e-12);
}
