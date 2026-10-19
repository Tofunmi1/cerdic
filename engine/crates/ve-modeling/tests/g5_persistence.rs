//! Persistence, history, group and instance coverage for a generated sweep.
use serde_json::json;
use ve_graph::{Document, Outcome, Session, builtin_registry};
use ve_modeling::{Evaluator, Quality};

fn ring_group() -> Session {
    let mut session = Session::with_builtin(5);
    let commands = json!({
        "doc": session.document().id,
        "base_revision": 0,
        "transaction": "t_g5_group",
        "commands": [
            {"add_node": {"op": "profile.circle@1", "params": {"radius": 0.1, "segments": 16}, "label": "section", "alias": "$section"}},
            {"add_node": {"op": "curve.circle@1", "params": {"center": [2,0,0], "radius": 1, "segments": 16}, "label": "path", "alias": "$path"}},
            {"add_node": {"op": "solid.sweep@1", "label": "sweep", "alias": "$sweep"}},
            {"connect": {"from": {"node": "$section", "port": "out"}, "to": {"node": "$sweep", "port": "profile"}}},
            {"connect": {"from": {"node": "$path", "port": "out"}, "to": {"node": "$sweep", "port": "curve"}}},
            {"create_group": {"name": "ring", "members": ["$section", "$path", "$sweep"], "output": "$sweep", "alias": "$group"}},
            {"expose_param": {"group": "$group", "name": "radius", "node": "$section", "param": "radius"}},
            {"add_instance": {"group": "$group", "label": "left", "alias": "$left"}},
            {"add_instance": {"group": "$group", "label": "right", "alias": "$right"}},
            {"set_output": {"name": "left", "from": {"node": "$left", "port": "out"}}},
            {"set_output": {"name": "right", "from": {"node": "$right", "port": "out"}}}
        ]
    });
    assert!(matches!(
        session.apply_json(&commands.to_string()),
        Outcome::Committed { .. }
    ));
    session
}

#[test]
fn g5_11_instance_override_and_definition_edit_have_distinct_effects() {
    let mut session = ring_group();
    let outputs = evaluate(&session);
    let initial = outputs.parts[0].volume;
    assert!((outputs.parts[1].volume - initial).abs() < 1e-12);
    let right = session
        .document()
        .nodes
        .iter()
        .find(|node| node.label.as_deref() == Some("right"))
        .unwrap();
    let override_command = json!({
        "doc": session.document().id,
        "base_revision": session.document().revision,
        "transaction": "t_g5_override",
        "commands": [{"set_instance_param": {"node": right.id, "param": "radius", "value": 0.13}}]
    });
    assert!(matches!(
        session.apply_json(&override_command.to_string()),
        Outcome::Committed { .. }
    ));
    let overridden = evaluate(&session);
    assert!(overridden.parts[1].volume > overridden.parts[0].volume);

    let section = session
        .document()
        .nodes
        .iter()
        .find(|node| node.op.family().as_str() == "profile.circle" && node.group.is_some())
        .unwrap();
    let definition = json!({
        "doc": session.document().id,
        "base_revision": session.document().revision,
        "transaction": "t_g5_definition",
        "commands": [{"set_param": {"node": section.id, "param": "radius", "value": 0.12}}]
    });
    assert!(matches!(
        session.apply_json(&definition.to_string()),
        Outcome::Committed { .. }
    ));
    let edited = evaluate(&session);
    assert!(edited.parts[0].volume > overridden.parts[0].volume);
    assert!((edited.parts[1].volume - overridden.parts[1].volume).abs() < 1e-12);
}

#[expect(
    clippy::unwrap_used,
    reason = "the persistence test helper must fail on evaluation errors"
)]
fn evaluate(session: &Session) -> ve_modeling::Evaluation {
    Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .unwrap()
}

#[test]
fn g5_11_sweep_survives_save_reload_history_group_and_instances() {
    let session = ring_group();
    let original = evaluate(&session);
    assert_eq!(original.parts.len(), 2);
    assert_eq!(
        original.evaluated_nodes, 3,
        "group prototype evaluated once"
    );

    let saved = session.document().canonical_json();
    let loaded: Document = serde_json::from_str(&saved).unwrap();
    assert_eq!(loaded.canonical_json(), saved);
    let mut restored = Session::new(builtin_registry(), loaded);
    let reloaded = evaluate(&restored);
    for (before, after) in original.parts.iter().zip(&reloaded.parts) {
        assert_eq!(before.positions, after.positions);
        assert_eq!(before.indices, after.indices);
        assert!((before.volume - after.volume).abs() < 1e-12);
    }

    let before_edit = restored.document().clone();
    let section = restored
        .document()
        .nodes
        .iter()
        .find(|node| node.op.family().as_str() == "profile.circle" && node.group.is_some())
        .unwrap();
    let edit = json!({
        "doc": restored.document().id,
        "base_revision": restored.document().revision,
        "transaction": "t_g5_radius",
        "commands": [{"set_param": {"node": section.id, "param": "radius", "value": 0.12}}]
    });
    assert!(matches!(
        restored.apply_json(&edit.to_string()),
        Outcome::Committed { .. }
    ));
    let widened = evaluate(&restored);
    assert!(
        widened
            .parts
            .iter()
            .all(|part| part.volume > original.parts[0].volume)
    );
    restored.undo().unwrap();
    assert!(restored.document().same_content(&before_edit));
    restored.redo().unwrap();
    assert!((evaluate(&restored).parts[0].volume - widened.parts[0].volume).abs() < 1e-12);
}
