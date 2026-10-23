//! A failure names the node the way the program wrote it (its label), so a model can find the line without bisecting ids.

use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn fails(text: &str) -> Box<ve_graph::Diagnostic> {
    let parsed = parse_text(&builtin_registry(), text).expect("the program parses");
    let mut session = Session::with_builtin(1);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_names").expect("a transaction id"),
        label: None,
        commands: parsed.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .expect_err("the evaluation fails")
}

#[test]
fn g18_01_a_failing_node_is_named_by_its_label_in_the_message() {
    let error = fails(
        "wall = solid.box@1(size=[40, 3, 0.3])\nrough = xform.noise@1(wall, amplitude=0.05, scale=1, tolerance=0.001)\noutput wall = rough\n",
    );
    assert!(
        error.message.contains("rough"),
        "the message names the node: {}",
        error.message
    );
}

#[test]
fn g18_03_a_union_of_one_solid_is_that_solid() {
    // Reusable pieces build the list of things to join, and a list may have one member.
    let parsed = parse_text(
        &builtin_registry(),
        "a = solid.box@1(size=[1, 1, 1])\nj = solid.union@1(a)\noutput o = j\n",
    )
    .expect("a union of one input parses");
    let mut session = Session::with_builtin(1);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_union1").expect("a transaction id"),
        label: None,
        commands: parsed.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .expect("it evaluates");
    assert_eq!(result.parts[0].indices.len() / 3, 12, "still a box");
}

#[test]
fn g18_02_a_failure_inside_a_group_names_the_instance() {
    let error = fails(
        "group slab\n  src = group.input@1(name=\"src\")\n  n = xform.noise@1(src, amplitude=0.05, scale=1, tolerance=0.001)\n  expose x = n.amplitude\n  result n\nend\nwall = solid.box@1(size=[40, 3, 0.3])\nthe_slab = slab(src=wall)\noutput w = the_slab\n",
    );
    assert!(
        error.message.contains("the_slab, member n"),
        "the instance and the member are named: {}",
        error.message
    );
}
