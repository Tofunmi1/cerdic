//! V1: the history a person sees: one labelled entry per transaction, what each touched, and where
//! undo and redo stand.
#![expect(
    clippy::unwrap_used,
    reason = "test helpers sit outside #[test] functions; failing loudly is the point"
)]

use ve_graph::{Command, CommandBatch, Outcome, Session, TransactionId, Value};

/// Adds one box in its own labelled transaction.
fn commit_labelled(session: &mut Session, label: Option<&str>) {
    let size = Value::List(vec![Value::Num(1.0); 3]);
    let batch = CommandBatch {
        doc: session.document().id.clone(),
        base_revision: session.document().revision,
        transaction: TransactionId::parse(&format!("t_hist{}", session.document().revision))
            .unwrap(),
        label: label.map(str::to_owned),
        commands: vec![Command::AddNode {
            op: "solid.box@1".to_owned(),
            params: [("size".to_owned(), size)].into(),
            label: None,
            alias: None,
            group: None,
        }],
    };
    assert!(matches!(session.apply(&batch), Outcome::Committed { .. }));
}

fn labels(items: &[ve_graph::HistoryItem]) -> Vec<&str> {
    items.iter().map(|item| item.label.as_str()).collect()
}

#[test]
fn a_new_session_has_no_history() {
    let history = Session::with_builtin(1).history();
    assert!(history.undo.is_empty() && history.redo.is_empty());
}

#[test]
fn every_transaction_is_listed_oldest_first_with_its_label() {
    let mut s = Session::with_builtin(1);
    commit_labelled(&mut s, Some("Add base"));
    commit_labelled(&mut s, Some("Add lid"));
    commit_labelled(&mut s, None);
    let history = s.history();
    assert_eq!(labels(&history.undo), ["Add base", "Add lid", "Edit"]);
    assert!(history.redo.is_empty());
    let first = &history.undo[0];
    assert_eq!(
        first.changed,
        vec![s.document().nodes[0].id.clone()],
        "an entry names what it touched"
    );
}

#[test]
fn undone_entries_wait_in_the_redo_list_with_the_next_one_first() {
    let mut s = Session::with_builtin(1);
    for label in ["one", "two", "three"] {
        commit_labelled(&mut s, Some(label));
    }
    s.undo().unwrap();
    s.undo().unwrap();
    let history = s.history();
    assert_eq!(labels(&history.undo), ["one"]);
    assert_eq!(
        labels(&history.redo),
        ["two", "three"],
        "redo goes to 'two' first"
    );
    s.redo().unwrap();
    let history = s.history();
    assert_eq!(labels(&history.undo), ["one", "two"]);
    assert_eq!(labels(&history.redo), ["three"]);
}

#[test]
fn a_new_edit_after_undo_clears_the_redo_list() {
    let mut s = Session::with_builtin(1);
    commit_labelled(&mut s, Some("one"));
    commit_labelled(&mut s, Some("two"));
    s.undo().unwrap();
    commit_labelled(&mut s, Some("three"));
    let history = s.history();
    assert_eq!(labels(&history.undo), ["one", "three"]);
    assert!(history.redo.is_empty());
}

#[test]
fn a_long_label_is_cut_to_a_readable_length() {
    let mut s = Session::with_builtin(1);
    commit_labelled(&mut s, Some(&"x".repeat(500)));
    assert!(s.history().undo[0].label.chars().count() <= 80);
}
