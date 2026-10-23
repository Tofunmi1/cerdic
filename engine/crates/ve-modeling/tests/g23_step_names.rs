//! While an evaluation runs step by step, the evaluator says which node the next step will run (its operator and the name the program
//! gave it), so a caller can time each node and a model can be told which of its lines is slow. Written before the body; do not weaken.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup unwraps data that is known to be valid"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn session(source: &str) -> Session {
    let commands = parse_text(&builtin_registry(), source).unwrap().commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g23_names").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    session
}

#[test]
fn each_step_names_the_node_it_is_about_to_run() {
    let session = session(
        "wall = solid.box@1(size=[2, 1, 0.2])\nlifted = xform.move@1(wall, by=[0, 1, 0])\noutput lifted = lifted\n",
    );
    let mut evaluator = Evaluator::default();
    evaluator.begin(session.document(), Quality::Final).unwrap();
    let mut seen = Vec::new();
    loop {
        let next = evaluator.next_node();
        let done = evaluator.step().unwrap().is_some();
        if let Some(next) = next {
            seen.push((next.operator, next.label));
        }
        if done {
            break;
        }
    }
    assert_eq!(seen.len(), 2, "two nodes ran: {seen:?}");
    assert_eq!(seen[0], ("solid.box".to_owned(), Some("wall".to_owned())));
    assert_eq!(
        seen[1],
        ("xform.move".to_owned(), Some("lifted".to_owned()))
    );
}

#[test]
fn nothing_is_next_before_a_job_begins_and_after_the_last_node() {
    let session = session("wall = solid.box@1(size=[2, 1, 0.2])\noutput wall = wall\n");
    let mut evaluator = Evaluator::default();
    assert!(evaluator.next_node().is_none(), "no job yet");
    evaluator.begin(session.document(), Quality::Final).unwrap();
    assert!(evaluator.next_node().is_some());
    assert!(evaluator.step().unwrap().is_none());
    assert!(
        evaluator.next_node().is_none(),
        "every node has run; the next step finishes the scene"
    );
    assert!(evaluator.step().unwrap().is_some());
}

#[test]
fn a_node_inside_a_group_is_named_with_its_instance() {
    let session = session(
        "group pillar\n  base = solid.box@1(size=[1, 2, 1])\n  result base\nend\nhall = pillar()\noutput hall = hall\n",
    );
    let mut evaluator = Evaluator::default();
    evaluator.begin(session.document(), Quality::Final).unwrap();
    let next = evaluator.next_node().unwrap();
    assert_eq!(next.operator, "solid.box");
    assert!(
        next.label
            .as_deref()
            .is_some_and(|l| l.contains("hall") && l.contains("base")),
        "named by instance and member: {:?}",
        next.label
    );
}
