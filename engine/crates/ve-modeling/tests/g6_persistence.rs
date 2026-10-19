//! G6-17: a polygon-editing graph saves, reloads, undoes, redoes, groups and instances like any other.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "the helpers unwrap fixtures that are known to be valid and report failures by panicking"
)]
use serde_json::json;
use ve_graph::{
    CommandBatch, Document, Outcome, Session, TransactionId, builtin_registry, parse_text,
};
use ve_modeling::{Evaluation, Evaluator, Quality};

const TRAY: &str = "group tray\n\
    box = solid.box@1(size=[1, 1, 1])\n\
    mesh = poly.from_solid@1(box)\n\
    flat = poly.merge_coplanar@1(mesh)\n\
    top = poly.select@1(flat, by=\"normal\", direction=[0, 1, 0], angle=0.05, expect=\"one\")\n\
    raised = poly.extrude_faces@1(flat, top, offset=[0, 0.2, 0])\n\
    solid = poly.to_solid@1(raised)\n\
    expose lift = raised.offset\n\
    result solid\n\
    end\n\
    left = tray()\n\
    right = tray(lift=[0, 0.4, 0])\n\
    output left\n\
    output right";

fn session() -> Session {
    let batch = parse_text(&builtin_registry(), TRAY).unwrap();
    let mut session = Session::with_builtin(7);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g6_persist").unwrap(),
        label: None,
        commands: batch.commands,
    });
    assert!(
        matches!(outcome, Outcome::Committed { .. }),
        "commits: {outcome:?}"
    );
    session
}

fn evaluate(session: &Session) -> Evaluation {
    Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .unwrap_or_else(|e| panic!("{}: {}", e.code, e.message))
}

fn apply(session: &mut Session, transaction: &str, command: serde_json::Value) {
    let batch = json!({
        "doc": session.document().id,
        "base_revision": session.document().revision,
        "transaction": transaction,
        "commands": [command]
    });
    assert!(matches!(
        session.apply_json(&batch.to_string()),
        Outcome::Committed { .. }
    ));
}

#[test]
fn g6_17_instances_share_one_definition_and_an_override_changes_only_its_own() {
    let mut session = session();
    let first = evaluate(&session);
    assert_eq!(first.parts.len(), 2);
    assert!(
        (first.parts[0].volume - 1.2).abs() < 1.0e-5,
        "default lift 0.2"
    );
    assert!(
        (first.parts[1].volume - 1.4).abs() < 1.0e-5,
        "overridden lift 0.4"
    );

    let raised = session
        .document()
        .nodes
        .iter()
        .find(|n| n.label.as_deref() == Some("raised") && n.group.is_some())
        .unwrap()
        .id
        .clone();
    apply(
        &mut session,
        "t_g6_definition",
        json!({"set_param": {"node": raised, "param": "offset", "value": [0, 0.3, 0]}}),
    );
    let edited = evaluate(&session);
    assert!(
        (edited.parts[0].volume - 1.3).abs() < 1.0e-5,
        "the definition edit reaches the plain instance"
    );
    assert!(
        (edited.parts[1].volume - 1.4).abs() < 1.0e-5,
        "the overridden instance keeps its own value"
    );
}

#[test]
fn g6_17_a_polygon_graph_survives_save_reload_undo_and_redo() {
    let mut original = session();
    let before = evaluate(&original);

    let saved = original.document().canonical_json();
    let loaded: Document = serde_json::from_str(&saved).unwrap();
    assert_eq!(loaded.canonical_json(), saved, "canonical form is stable");
    let restored = Session::new(builtin_registry(), loaded);
    let after = evaluate(&restored);
    for (a, b) in before.parts.iter().zip(&after.parts) {
        assert_eq!(a.positions, b.positions);
        assert_eq!(a.indices, b.indices);
    }

    let snapshot = original.document().clone();
    let raised = original
        .document()
        .nodes
        .iter()
        .find(|n| n.label.as_deref() == Some("raised") && n.group.is_some())
        .unwrap()
        .id
        .clone();
    apply(
        &mut original,
        "t_g6_edit",
        json!({"set_param": {"node": raised, "param": "offset", "value": [0, 0.5, 0]}}),
    );
    let widened = evaluate(&original);
    assert!(widened.parts[0].volume > before.parts[0].volume);
    original.undo().unwrap();
    assert!(original.document().same_content(&snapshot));
    assert!((evaluate(&original).parts[0].volume - before.parts[0].volume).abs() < 1.0e-12);
    original.redo().unwrap();
    assert!((evaluate(&original).parts[0].volume - widened.parts[0].volume).abs() < 1.0e-12);
}

#[test]
fn g6_17_the_shared_definition_is_evaluated_once() {
    let session = session();
    let result = evaluate(&session);
    // box, from_solid, merge_coplanar, select, extrude, to_solid: the six prototype nodes run once
    // however many instances use them, plus the one instance whose lift differs.
    assert!(
        result.evaluated_nodes <= 12,
        "evaluated {} nodes",
        result.evaluated_nodes
    );
}
