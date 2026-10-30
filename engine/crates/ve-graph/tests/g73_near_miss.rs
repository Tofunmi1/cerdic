//! Near-misses a model makes are accepted or answered with the fix: a single number for a vector, and a "did you mean" for names.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "case helpers unwrap fixtures that are known to be valid and fail loudly otherwise"
)]
use ve_graph::{
    CommandBatch, Diagnostic, Document, Outcome, Session, TransactionId, builtin_registry,
    parse_text,
};

fn build(text: &str) -> Result<Document, Vec<Diagnostic>> {
    let registry = builtin_registry();
    let batch = parse_text(&registry, text)?;
    let mut session = Session::with_builtin(5);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_text").unwrap(),
        label: None,
        commands: batch.commands.clone(),
    });
    match outcome {
        Outcome::Committed { .. } => Ok(session.document().clone()),
        Outcome::Rejected { diagnostics } => {
            Err(diagnostics.into_iter().map(|d| batch.locate(d)).collect())
        }
        other => panic!("{other:?}"),
    }
}

fn doc(text: &str) -> Document {
    build(text).unwrap_or_else(|e| panic!("{text}\n{e:?}"))
}

fn first_error(text: &str) -> Diagnostic {
    build(text).unwrap_err().remove(0)
}

fn param(doc: &Document, label: &str, key: &str) -> String {
    let node = doc
        .nodes
        .iter()
        .find(|n| n.label.as_deref() == Some(label))
        .unwrap();
    format!("{:?}", node.params[key])
}

#[test]
fn one_number_fills_a_three_component_length() {
    let broadcast = doc("a = solid.box(size=2)\noutput o = a");
    let written = doc("a = solid.box(size=[2, 2, 2])\noutput o = a");
    assert_eq!(param(&broadcast, "a", "size"), param(&written, "a", "size"));
}

#[test]
fn one_number_fills_a_move_offset_with_units() {
    let broadcast = doc("a = solid.box(size=[1, 1, 1])\nb = xform.move(a, by=50cm)\noutput o = b");
    let written =
        doc("a = solid.box(size=[1, 1, 1])\nb = xform.move(a, by=[0.5, 0.5, 0.5])\noutput o = b");
    assert_eq!(param(&broadcast, "b", "by"), param(&written, "b", "by"));
}

#[test]
fn a_single_number_for_a_list_of_the_wrong_length_is_still_an_error_when_the_range_fails() {
    let e = first_error("a = solid.box(size=-1)");
    assert!(
        e.code.starts_with("param.") || e.code.starts_with("parse."),
        "{e:?}"
    );
}

#[test]
fn a_list_of_the_wrong_length_is_still_refused_and_says_how_many() {
    let e = first_error("a = solid.box(size=[1, 2])");
    assert!(e.message.contains('3'), "{}", e.message);
}

#[test]
fn a_misspelled_operator_suggests_the_closest_ones() {
    let e = first_error("a = solid.bx(size=[1, 1, 1])");
    let hint = e.suggestion.clone().unwrap_or_default() + &e.message;
    assert!(hint.contains("solid.box"), "{hint}");
}

#[test]
fn a_misspelled_parameter_suggests_the_closest_one() {
    let e = first_error("a = solid.box(sizes=[1, 1, 1])");
    assert_eq!(e.code, "parse.unknown_param");
    let hint = e.suggestion.clone().unwrap_or_default();
    assert!(hint.contains("size"), "{hint}");
    assert!(hint.to_lowercase().contains("did you mean"), "{hint}");
}

#[test]
fn a_misspelled_node_name_suggests_the_closest_one() {
    let e = first_error("post = solid.box(size=[1, 1, 1])\nb = xform.move(psot, by=[0, 1, 0])");
    let hint = e.suggestion.clone().unwrap_or_default() + &e.message;
    assert!(hint.contains("post"), "{hint}");
    assert!(hint.to_lowercase().contains("did you mean"), "{hint}");
}

#[test]
fn a_wrong_case_enum_value_is_accepted() {
    let upper = doc("a = solid.box(size=[1, 1, 1])\nb = xform.mirror(a, axis=\"X\")\noutput o = b");
    let lower = doc("a = solid.box(size=[1, 1, 1])\nb = xform.mirror(a, axis=\"x\")\noutput o = b");
    assert_eq!(param(&upper, "b", "axis"), param(&lower, "b", "axis"));
}

#[test]
fn an_unknown_enum_value_lists_the_choices() {
    let e = first_error("a = solid.box(size=[1, 1, 1])\nb = xform.mirror(a, axis=\"q\")");
    assert!(
        e.message.contains('x') && e.message.to_lowercase().contains("axis"),
        "{}",
        e.message
    );
}

#[test]
fn a_far_off_name_gets_no_made_up_suggestion() {
    let e = first_error("a = solid.box(size=[1, 1, 1])\nb = xform.move(zzzzzz, by=[0, 1, 0])");
    let hint = e.suggestion.clone().unwrap_or_default();
    assert!(!hint.to_lowercase().contains("did you mean"), "{hint}");
}
