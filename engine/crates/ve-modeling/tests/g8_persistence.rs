//! G8-16: field nodes keep their data and stable IDs through groups, instances and document history.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "the fixed field persistence graph must be valid for its regression case"
)]
use serde_json::json;
use ve_graph::{
    CommandBatch, Document, Outcome, Session, TransactionId, builtin_registry, parse_text,
};
use ve_modeling::{Evaluator, Quality};

const PROGRAM: &str = "group field_sculpt\n\
    sphere = field.sphere@1(radius=0.5)\n\
    solid = field.to_solid@1(sphere, min=[-1, -1, -1], max=[1, 1, 1], edge_length=0.14)\n\
    expose radius = sphere.radius\n\
    result solid\n\
    end\n\
    small = field_sculpt()\n\
    large = field_sculpt(radius=0.7)\n\
    output small\n\
    output large";

fn session() -> Session {
    let batch = parse_text(&builtin_registry(), PROGRAM)
        .unwrap_or_else(|errors| panic!("parse errors: {errors:?}"));
    let mut session = Session::with_builtin(54);
    let result = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g8_field_group").unwrap(),
        label: None,
        commands: batch.commands,
    });
    assert!(matches!(result, Outcome::Committed { .. }), "{result:?}");
    session
}

fn evaluate(session: &Session) -> ve_modeling::Evaluation {
    Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .unwrap_or_else(|error| panic!("{}: {}", error.code, error.message))
}

#[test]
fn g8_16_field_groups_instances_save_reload_undo_and_redo_keep_ids_and_meshes() {
    let mut source = session();
    let initial = evaluate(&source);
    assert_eq!(initial.parts.len(), 2);
    assert!(initial.parts[1].volume > initial.parts[0].volume);
    let ids: Vec<_> = source
        .document()
        .nodes
        .iter()
        .map(|node| node.id.clone())
        .collect();

    let saved = source.document().canonical_json();
    let restored: Document = serde_json::from_str(&saved).unwrap();
    assert_eq!(restored.canonical_json(), saved);
    let loaded = Session::new(builtin_registry(), restored);
    let round_trip = evaluate(&loaded);
    for (before, after) in initial.parts.iter().zip(&round_trip.parts) {
        assert_eq!(before.positions, after.positions);
        assert_eq!(before.indices, after.indices);
    }

    let large = source
        .document()
        .nodes
        .iter()
        .find(|node| node.label.as_deref() == Some("large") && node.instance.is_some())
        .unwrap()
        .id
        .clone();
    let edit = json!({
        "doc": source.document().id,
        "base_revision": source.document().revision,
        "transaction": "t_g8_field_override",
        "commands": [{"set_instance_param": {"node": large, "param": "radius", "value": 0.8}}]
    });
    assert!(matches!(
        source.apply_json(&edit.to_string()),
        Outcome::Committed { .. }
    ));
    let edited = evaluate(&source);
    assert!(edited.parts[1].volume > initial.parts[1].volume);
    assert_eq!(
        source
            .document()
            .nodes
            .iter()
            .map(|node| node.id.clone())
            .collect::<Vec<_>>(),
        ids,
        "an instance override must preserve every authored ID"
    );
    assert!(source.undo().is_some());
    let undone = evaluate(&source);
    assert_eq!(undone.parts[1].positions, initial.parts[1].positions);
    assert!(source.redo().is_some());
    let redone = evaluate(&source);
    assert_eq!(redone.parts[1].positions, edited.parts[1].positions);
}
