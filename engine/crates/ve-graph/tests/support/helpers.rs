//! Shared helpers for the case tests: build batches, commit or reject them, and name nodes.

use std::sync::atomic::{AtomicU64, Ordering};

use ve_graph::{
    Alias, Command, CommandBatch, Diagnostic, InputRef, NodeId, NodeRef, OutRef, Outcome, Session,
    TransactionId, Value,
};

static KEY: AtomicU64 = AtomicU64::new(0);

/// A batch for the session's current revision with a fresh transaction key.
pub(crate) fn batch(session: &Session, commands: Vec<Command>) -> CommandBatch {
    let key = KEY.fetch_add(1, Ordering::Relaxed);
    CommandBatch {
        doc: session.document().id.clone(),
        base_revision: session.document().revision,
        transaction: TransactionId::parse(&format!("t_case{key}")).unwrap(),
        label: None,
        commands,
    }
}

/// Applies the commands and asserts they committed.
pub(crate) fn commit(session: &mut Session, commands: Vec<Command>) -> Outcome {
    let b = batch(session, commands);
    let outcome = session.apply(&b);
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    outcome
}

/// Applies the commands, asserts they were rejected and changed nothing, and returns why.
pub(crate) fn reject(session: &mut Session, commands: Vec<Command>) -> Vec<Diagnostic> {
    let before = session.document().clone();
    let b = batch(session, commands);
    let Outcome::Rejected { diagnostics } = session.apply(&b) else {
        panic!("expected a rejection");
    };
    assert_eq!(
        session.document(),
        &before,
        "a rejected batch must change nothing"
    );
    diagnostics
}

pub(crate) fn num(n: f64) -> Value {
    Value::Num(n)
}

pub(crate) fn vec3(x: f64, y: f64, z: f64) -> Value {
    Value::List(vec![num(x), num(y), num(z)])
}

pub(crate) fn add_as(op: &str, params: &[(&str, Value)], alias: Option<&str>) -> Command {
    Command::AddNode {
        op: op.to_owned(),
        params: params
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect(),
        label: None,
        alias: alias.map(|a| Alias::parse(a).unwrap()),
        group: None,
    }
}

pub(crate) fn add(op: &str, params: &[(&str, Value)]) -> Command {
    add_as(op, params, None)
}

pub(crate) fn add_box() -> Command {
    add("solid.box@1", &[("size", vec3(1.0, 1.0, 1.0))])
}

pub(crate) fn add_union() -> Command {
    add("solid.union@1", &[])
}

pub(crate) fn last_id(session: &Session) -> NodeId {
    session.document().nodes.last().unwrap().id.clone()
}

pub(crate) fn node(id: &NodeId) -> NodeRef {
    NodeRef::Id(id.clone())
}

pub(crate) fn alias(name: &str) -> NodeRef {
    NodeRef::Alias(Alias::parse(name).unwrap())
}

pub(crate) fn input(target: NodeRef) -> InputRef {
    InputRef {
        node: target,
        port: "in".into(),
        index: None,
    }
}

pub(crate) fn link(from: &NodeId, to: &NodeId) -> Command {
    Command::Connect {
        from: OutRef::out(from.clone()),
        to: input(node(to)),
    }
}
