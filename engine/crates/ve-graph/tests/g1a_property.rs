//! Acceptance cases G1a-19 and G1a-20: random command sequences.
//!
//! Commands are generated from raw numbers interpreted against the current document, so they are a
//! mix of valid edits and mistakes (unknown nodes, bad values, cycles). Rejected batches must change
//! nothing; committed ones must undo and replay exactly.
#![cfg(not(target_family = "wasm"))]
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test helpers sit outside #[test] functions; failing loudly is the point"
)]

use proptest::prelude::*;
use ve_graph::{
    Alias, Command, CommandBatch, Document, InputRef, NodeId, NodeRef, OutRef, Outcome, Session,
    TransactionId, Value,
};

const OPS: [&str; 5] = [
    "solid.box@1",
    "solid.sphere@1",
    "solid.union@1",
    "xform.move@1",
    "solid.difference@1",
];

fn params_for(op: &str, x: f64) -> std::collections::BTreeMap<String, Value> {
    let v3 = |a: f64| Value::List(vec![Value::Num(a); 3]);
    match op {
        "solid.box@1" => [("size".to_owned(), v3(x.abs() + 0.1))].into(),
        "solid.sphere@1" => [("radius".to_owned(), Value::Num(x.abs() + 0.1))].into(),
        "xform.move@1" => [("by".to_owned(), v3(x))].into(),
        _ => std::collections::BTreeMap::new(),
    }
}

/// Turns raw numbers into a command against the nodes that exist now.
fn command(doc: &Document, kind: u8, a: u8, b: u8, x: f64) -> Command {
    let pick = |n: u8| -> Option<NodeId> {
        (!doc.nodes.is_empty()).then(|| doc.nodes[usize::from(n) % doc.nodes.len()].id.clone())
    };
    let ghost = NodeId::parse("n_zzzzzzzz").unwrap_or_else(|_| unreachable_id());
    let target = |n: u8| NodeRef::Id(pick(n).unwrap_or_else(|| ghost.clone()));
    match kind % 8 {
        0 | 1 => {
            let op = OPS[usize::from(a) % OPS.len()];
            Command::AddNode {
                op: op.to_owned(),
                params: params_for(op, x),
                label: None,
                alias: None,
            }
        }
        2 => Command::RemoveNode {
            node: target(a),
            cascade: b.is_multiple_of(2),
        },
        3 => Command::Connect {
            from: OutRef::out(pick(a).unwrap_or_else(|| ghost.clone())),
            to: InputRef {
                node: target(b),
                port: "in".into(),
                index: None,
            },
        },
        4 => Command::SetParam {
            node: target(a),
            param: "size".into(),
            value: Value::Num(x),
        },
        5 => Command::SetLabel {
            node: target(a),
            label: Some(format!("n{b}")),
        },
        6 => Command::EditorSetPosition {
            node: target(a),
            x,
            y: x * 2.0,
        },
        _ => Command::Disconnect {
            to: InputRef {
                node: target(a),
                port: "in".into(),
                index: Some(usize::from(b % 3)),
            },
        },
    }
}

fn unreachable_id() -> NodeId {
    NodeId::parse("n_00000000").unwrap()
}

fn batch(doc: &Document, n: usize, commands: Vec<Command>) -> CommandBatch {
    CommandBatch {
        doc: doc.id.clone(),
        base_revision: doc.revision,
        transaction: TransactionId::parse(&format!("t_prop{n}")).unwrap(),
        label: None,
        commands,
    }
}

fn run(seed: u64, steps: &[(u8, u8, u8, f64, u8)]) -> (Session, Vec<CommandBatch>, Vec<Document>) {
    let mut session = Session::with_builtin(seed);
    let (mut committed, mut states) = (Vec::new(), vec![session.document().clone()]);
    for (n, (kind, a, b, x, count)) in steps.iter().enumerate() {
        let doc = session.document().clone();
        let commands: Vec<Command> = (0..=(count % 3))
            .map(|i| command(&doc, kind.wrapping_add(i), a.wrapping_add(i), *b, *x))
            .collect();
        let b = batch(&doc, n, commands);
        match session.apply(&b) {
            Outcome::Committed { .. } => {
                committed.push(b);
                states.push(session.document().clone());
            }
            Outcome::Rejected { .. } => {
                assert_eq!(session.document(), &doc, "a rejection changed the document");
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    (session, committed, states)
}

fn steps() -> impl Strategy<Value = Vec<(u8, u8, u8, f64, u8)>> {
    proptest::collection::vec(
        (
            any::<u8>(),
            any::<u8>(),
            any::<u8>(),
            -5.0..5.0_f64,
            any::<u8>(),
        ),
        0..40,
    )
}

proptest! {
    /// G1a-19: undoing every committed batch gives back the empty document.
    #[test]
    fn g1a_19_undoing_everything_gives_the_empty_document(seed in 0..1000_u64, steps in steps()) {
        let (mut session, committed, _) = run(seed, &steps);
        for _ in &committed {
            prop_assert!(session.undo().is_some());
        }
        prop_assert!(session.undo().is_none());
        prop_assert!(session.document().same_content(&Document::new(seed)));
    }

    /// G1a-20: replaying the committed batches reproduces the document at every revision.
    #[test]
    fn g1a_20_replaying_history_reproduces_every_revision(seed in 0..1000_u64, steps in steps()) {
        let (_, committed, states) = run(seed, &steps);
        let mut replay = Session::with_builtin(seed);
        for (i, b) in committed.iter().enumerate() {
            let mut b = b.clone();
            b.base_revision = replay.document().revision;
            let outcome = replay.apply(&b);
            let applied = matches!(outcome, Outcome::Committed { .. });
            prop_assert!(applied, "{:?}", outcome);
            prop_assert_eq!(replay.document(), &states[i + 1]);
        }
    }

    /// Redo after full undo returns to the final document.
    #[test]
    fn undo_then_redo_everything_returns_to_the_end(seed in 0..1000_u64, steps in steps()) {
        let (mut session, committed, _) = run(seed, &steps);
        let end = session.document().clone();
        for _ in &committed {
            session.undo();
        }
        for _ in &committed {
            prop_assert!(session.redo().is_some());
        }
        prop_assert!(session.document().same_content(&end));
    }
}

#[test]
fn the_generator_commits_often_enough_to_mean_something() {
    // A fixed pseudo-random walk: most batches should commit, and some should be rejected.
    let steps: Vec<(u8, u8, u8, f64, u8)> = (0..60_u8)
        .map(|i| {
            (
                i.wrapping_mul(37),
                i.wrapping_mul(11),
                i.wrapping_mul(5),
                f64::from(i % 7) - 3.0,
                i,
            )
        })
        .collect();
    let (session, committed, _) = run(3, &steps);
    assert!(
        committed.len() >= 20,
        "only {} of 60 batches committed",
        committed.len()
    );
    assert!(
        committed.len() < 60,
        "nothing was ever rejected, so the mistakes are not being tested"
    );
    assert!(!session.document().nodes.is_empty());
    assert!(Alias::parse("$x").is_ok());
}
