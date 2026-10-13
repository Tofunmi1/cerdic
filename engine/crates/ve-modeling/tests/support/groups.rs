//! Helpers for the group evaluation tests: the wheel fixture shared with `ve-graph`, instances of it
//! each followed by a move, and evaluation of a session's document. Every test file that includes this
//! module uses all of it.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test helpers sit outside #[test] functions; failing loudly is the point"
)]

use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value as Json, json};
use ve_graph::{Command, CommandBatch, GroupId, NodeId, Outcome, Session, TransactionId};
use ve_modeling::{Evaluation, Evaluator, Quality};

static KEY: AtomicU64 = AtomicU64::new(0);
const WHEEL: &str = include_str!("../../../ve-graph/tests/fixtures/wheel_group.json");

/// Applies the commands (a JSON array) and asserts they committed.
pub(crate) fn commit(session: &mut Session, json: Json) {
    let commands: Vec<Command> = serde_json::from_value(json).unwrap();
    let batch = CommandBatch {
        doc: session.document().id.clone(),
        base_revision: session.document().revision,
        transaction: TransactionId::parse(&format!("t_g4e{}", KEY.fetch_add(1, Ordering::Relaxed)))
            .unwrap(),
        label: None,
        commands,
    };
    let outcome = session.apply(&batch);
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
}

/// The ID of the node with this label.
pub(crate) fn named(session: &Session, label: &str) -> NodeId {
    session
        .document()
        .nodes
        .iter()
        .find(|n| n.label.as_deref() == Some(label))
        .unwrap_or_else(|| panic!("no node labelled {label}"))
        .id
        .clone()
}

/// The wheel group and `count` instances of it, labelled `w0`, `w1`, ..., each moved along X by its
/// index by a node labelled `m0`, `m1`, ... (so each instance is read, and the moves are the parts).
pub(crate) fn wheels(count: u32) -> (Session, GroupId) {
    let mut s = Session::with_builtin(4);
    commit(&mut s, serde_json::from_str(WHEEL).unwrap());
    let group = s.document().groups[0].id.clone();
    let mut commands = Vec::new();
    for i in 0..count {
        let (w, m) = (format!("$w{i}"), format!("$m{i}"));
        commands.push(json!({"add_instance": {"group": group.as_str(), "label": format!("w{i}"), "alias": w}}));
        commands.push(json!({"add_node": {"op": "xform.move@1", "params": {"by": [f64::from(i), 0, 0]}, "label": format!("m{i}"), "alias": m}}));
        commands.push(json!({"connect": {"from": {"node": w, "port": "out"}, "to": {"node": m, "port": "in"}}}));
    }
    if !commands.is_empty() {
        commit(&mut s, Json::Array(commands));
    }
    (s, group)
}

/// Evaluates the session's document with the given evaluator (which keeps its cache between calls).
pub(crate) fn evaluate(evaluator: &mut Evaluator, session: &Session) -> Evaluation {
    evaluator
        .evaluate(session.document(), Quality::Final)
        .unwrap()
}

/// The volume of each part, in order.
pub(crate) fn volumes(evaluation: &Evaluation) -> Vec<f64> {
    evaluation.parts.iter().map(|p| p.volume).collect()
}
