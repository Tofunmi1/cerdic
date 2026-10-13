//! Shared helpers for the group tests: build batches from JSON, commit or refuse them, and name nodes.
//! Every test file that includes this module uses all of it.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test helpers sit outside #[test] functions; failing loudly is the point"
)]

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value as Json, json};
use ve_graph::{
    Command, CommandBatch, Diagnostic, GroupId, NodeId, Outcome, Session, TransactionId, Value,
};

static KEY: AtomicU64 = AtomicU64::new(0);
const WHEEL: &str = include_str!("../fixtures/wheel_group.json");

fn commands(json: Json) -> Vec<Command> {
    serde_json::from_value(json).unwrap()
}

/// A batch of the given commands against the session's current revision, with a fresh key.
pub(crate) fn batch(session: &Session, json: Json) -> CommandBatch {
    CommandBatch {
        doc: session.document().id.clone(),
        base_revision: session.document().revision,
        transaction: TransactionId::parse(&format!("t_g4b{}", KEY.fetch_add(1, Ordering::Relaxed)))
            .unwrap(),
        label: Some("test".into()),
        commands: commands(json),
    }
}

/// Applies the commands and asserts they committed.
pub(crate) fn commit(session: &mut Session, json: Json) {
    let outcome = session.apply(&batch(session, json));
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
}

/// Applies the commands, asserts they were refused and changed nothing, and returns why.
pub(crate) fn reject(session: &mut Session, json: Json) -> Vec<Diagnostic> {
    let before = session.document().clone();
    let Outcome::Rejected { diagnostics } = session.apply(&batch(session, json)) else {
        panic!("expected a rejection");
    };
    assert_eq!(
        session.document(),
        &before,
        "a refused batch must change nothing"
    );
    diagnostics
}

/// A session holding the wheel group (`fixtures/wheel_group.json`) and its ID.
pub(crate) fn wheel() -> (Session, GroupId) {
    let mut s = Session::with_builtin(4);
    commit(&mut s, serde_json::from_str(WHEEL).unwrap());
    let id = s.document().groups[0].id.clone();
    (s, id)
}

/// The ID of the node with this label.
pub(crate) fn named(s: &Session, label: &str) -> NodeId {
    s.document()
        .nodes
        .iter()
        .find(|n| n.label.as_deref() == Some(label))
        .unwrap_or_else(|| panic!("no node labelled {label}"))
        .id
        .clone()
}

/// One instance of `group` per label, in one batch.
pub(crate) fn add_instances(s: &mut Session, group: &GroupId, labels: &[&str]) {
    let adds: Vec<Json> = labels
        .iter()
        .map(|l| json!({"add_instance": {"group": group.as_str(), "label": l}}))
        .collect();
    commit(s, Json::Array(adds));
}

/// The overrides on the instance with this label.
pub(crate) fn overrides(s: &Session, label: &str) -> BTreeMap<String, Value> {
    let id = named(s, label);
    s.document()
        .node(&id)
        .unwrap()
        .instance
        .as_ref()
        .unwrap()
        .overrides
        .clone()
}
