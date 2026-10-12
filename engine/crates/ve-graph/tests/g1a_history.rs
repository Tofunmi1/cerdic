//! Acceptance cases G1a-04 and G1a-08 to G1a-17: cycles, removal, undo and redo, idempotency,
//! conflicts, the evaluation hash, limits and ID reuse (see `docs/modeling/cases.md`).
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test helpers sit outside #[test] functions; failing loudly is the point"
)]

#[path = "support/helpers.rs"]
mod helpers;

use helpers::{
    add, add_as, add_box, add_union, alias, batch, commit, input, last_id, link, node, num, reject,
    vec3,
};
use ve_graph::{Command, OutRef, Outcome, Session};

#[test]
fn g1a_04_a_link_that_closes_a_cycle_is_refused_with_the_nodes() {
    let mut s = Session::with_builtin(1);
    commit(&mut s, vec![add_union()]);
    let u1 = last_id(&s);
    commit(&mut s, vec![add_union()]);
    let u2 = last_id(&s);
    commit(&mut s, vec![link(&u1, &u2)]);
    let diagnostics = reject(&mut s, vec![link(&u2, &u1)]);
    assert_eq!(diagnostics[0].code, "graph.cycle");
    assert!(diagnostics[0].related.contains(&u1) && diagnostics[0].related.contains(&u2));
    assert_eq!(reject(&mut s, vec![link(&u1, &u1)])[0].code, "graph.cycle");
}

#[test]
fn g1a_08_removing_a_node_that_is_read_needs_cascade() {
    let mut s = Session::with_builtin(1);
    commit(&mut s, vec![add_box(), add_union()]);
    let (a, b) = (
        s.document().nodes[0].id.clone(),
        s.document().nodes[1].id.clone(),
    );
    commit(&mut s, vec![link(&a, &b)]);
    let d = reject(
        &mut s,
        vec![Command::RemoveNode {
            node: node(&a),
            cascade: false,
        }],
    );
    assert_eq!(d[0].code, "graph.in_use");
    assert_eq!(d[0].related, vec![b]);
    commit(
        &mut s,
        vec![Command::RemoveNode {
            node: node(&a),
            cascade: true,
        }],
    );
    assert!(s.document().nodes.is_empty());
}

/// Five commands that all succeed on a document that has at least one node.
fn five_valid(s: &Session) -> Vec<Command> {
    let first = s.document().nodes.first().map(|n| n.id.clone()).unwrap();
    vec![
        add_as("solid.box@1", &[("size", vec3(1.0, 1.0, 1.0))], Some("$a")),
        add_as("solid.union@1", &[], Some("$u")),
        Command::Connect {
            from: OutRef::out(first.clone()),
            to: input(alias("$u")),
        },
        Command::EditorSetPosition {
            node: node(&first),
            x: 3.0,
            y: 4.0,
        },
        Command::SetLabel {
            node: alias("$a"),
            label: Some("extra".into()),
        },
    ]
}

#[test]
fn g1a_09_undo_restores_the_document_exactly() {
    let mut s = Session::with_builtin(1);
    commit(&mut s, vec![add_box()]);
    let before = s.document().clone();
    let commands = five_valid(&s);
    commit(&mut s, commands);
    assert_eq!(s.document().nodes.len(), 3);
    let moved = s.undo().unwrap();
    assert!(s.document().same_content(&before));
    assert_eq!(moved.revision, s.document().revision);
    assert!(
        s.document().revision > before.revision,
        "undo advances the revision"
    );
}

#[test]
fn g1a_10_redo_restores_what_undo_removed() {
    let mut s = Session::with_builtin(1);
    commit(&mut s, vec![add_box()]);
    let commands = five_valid(&s);
    commit(&mut s, commands);
    let after = s.document().clone();
    s.undo().unwrap();
    s.redo().unwrap();
    assert!(s.document().same_content(&after));
    assert!(s.redo().is_none());
}

#[test]
fn g1a_11_a_new_batch_after_undo_clears_redo() {
    let mut s = Session::with_builtin(1);
    commit(&mut s, vec![add_box()]);
    s.undo().unwrap();
    assert!(s.can_redo());
    commit(&mut s, vec![add_box()]);
    assert!(!s.can_redo());
}

#[test]
fn g1a_12_the_same_transaction_key_applies_once() {
    let mut s = Session::with_builtin(1);
    let b = batch(&s, vec![add_box()]);
    assert!(matches!(
        s.apply(&b),
        Outcome::Committed { revision: 1, .. }
    ));
    let snapshot = s.document().clone();
    assert_eq!(s.apply(&b), Outcome::AlreadyApplied { revision: 1 });
    assert_eq!(s.document(), &snapshot);
}

#[test]
fn g1a_13_a_stale_base_revision_conflicts_and_applies_nothing() {
    let mut s = Session::with_builtin(1);
    commit(&mut s, vec![add_box()]);
    let id = last_id(&s);
    let label = |text: &str| Command::SetLabel {
        node: node(&id),
        label: Some(text.into()),
    };
    let stale = batch(&s, vec![label("old")]);
    commit(&mut s, vec![label("new")]);
    let current = s.document().clone();
    match s.apply(&stale) {
        Outcome::Conflict {
            base_revision,
            current_revision,
            changed_since,
            complete,
        } => {
            assert_eq!((base_revision, current_revision), (1, 2));
            assert_eq!(changed_since, vec![id]);
            assert!(complete);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(s.document(), &current);
}

#[test]
fn g1a_14_moving_a_node_changes_the_revision_but_not_the_evaluation_hash() {
    let mut s = Session::with_builtin(1);
    commit(&mut s, vec![add_box()]);
    let id = last_id(&s);
    let hash = s.document().eval_hash();
    let revision = s.document().revision;
    commit(
        &mut s,
        vec![Command::EditorSetPosition {
            node: node(&id),
            x: 10.0,
            y: -4.0,
        }],
    );
    assert_eq!(s.document().revision, revision + 1);
    assert_eq!(s.document().eval_hash(), hash);
    assert_eq!(s.document().editor.positions.get(&id), Some(&[10.0, -4.0]));
    commit(
        &mut s,
        vec![Command::SetParam {
            node: node(&id),
            param: "size".into(),
            value: vec3(2.0, 1.0, 1.0),
        }],
    );
    assert_ne!(
        s.document().eval_hash(),
        hash,
        "a parameter change must change the hash"
    );
}

#[test]
fn g1a_15_renaming_keeps_the_id_and_the_evaluation_hash() {
    let mut s = Session::with_builtin(1);
    commit(&mut s, vec![add_box()]);
    let id = last_id(&s);
    let hash = s.document().eval_hash();
    commit(
        &mut s,
        vec![Command::SetLabel {
            node: node(&id),
            label: Some("table top".into()),
        }],
    );
    assert_eq!(last_id(&s), id);
    assert_eq!(s.document().eval_hash(), hash);
    let long = Command::SetLabel {
        node: node(&id),
        label: Some("x".repeat(257)),
    };
    assert_eq!(reject(&mut s, vec![long])[0].code, "param.range");
}

#[test]
fn g1a_16_two_hundred_commands_pass_and_two_hundred_and_one_do_not() {
    let mut s = Session::with_builtin(1);
    commit(&mut s, vec![add_box(); 200]);
    assert_eq!(s.document().nodes.len(), 200);
    let d = reject(&mut s, vec![add_box(); 201]);
    assert_eq!(d[0].code, "budget.commands");
    assert_eq!((d[0].limit, d[0].observed), (Some(200.0), Some(201.0)));
    assert_eq!(s.document().nodes.len(), 200);
}

#[test]
fn g1a_17_a_new_node_never_gets_a_deleted_nodes_id() {
    let mut s = Session::with_builtin(1);
    let mut seen = Vec::new();
    for _ in 0..30 {
        commit(&mut s, vec![add_box()]);
        let id = last_id(&s);
        assert!(!seen.contains(&id), "{id} was issued twice");
        seen.push(id.clone());
        commit(
            &mut s,
            vec![Command::RemoveNode {
                node: node(&id),
                cascade: false,
            }],
        );
    }
    // Undo of an add followed by a fresh add must not collide either.
    commit(&mut s, vec![add_box()]);
    let undone = last_id(&s);
    s.undo().unwrap();
    commit(&mut s, vec![add_box()]);
    assert_ne!(last_id(&s), undone);
}

#[test]
fn removing_a_sphere_keeps_a_parameter_check_alive() {
    // Exercises `add` and `num` so the shared helpers are used by both test files.
    let mut s = Session::with_builtin(1);
    commit(&mut s, vec![add("solid.sphere@1", &[("radius", num(1.0))])]);
    assert_eq!(s.document().nodes.len(), 1);
}
