//! Loops over text: `for name in ["a", "b"]` and `for [name, x] in [["a", 2]]` bind a name to a piece of text. `{name}` inside a node name is
//! the text itself, `{name}` anywhere else is that text as a quoted string, and `${name}` inside a string is spliced as before. A text that
//! cannot be part of a node name is refused with the fix. Written before the body; do not weaken.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "fixture helpers fail loudly"
)]

use ve_graph::{
    CommandBatch, Diagnostic, Document, Outcome, Session, TransactionId, builtin_registry,
    parse_text, to_text,
};

fn build(text: &str) -> Document {
    let mut session = Session::with_builtin(5);
    let batch = parse_text(session.registry(), text).unwrap();
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_text_loops").unwrap(),
        label: None,
        commands: batch.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    session.document().clone()
}

fn written(text: &str) -> String {
    to_text(&builtin_registry(), &build(text)).unwrap()
}

fn error(text: &str) -> Diagnostic {
    parse_text(&builtin_registry(), text).unwrap_err().remove(0)
}

const FRAMES: &str = "\
for [name, x] in [[\"alpha\", 1], [\"beta\", 2]]
  b_{name} = solid.box@1(size=[{x}, 1, 1])
  f_{name} = frame.add@1(b_{name}, name={name}, position=[{x}, 0, 0], rotation=[0, 0, 0])
  output o_{name} = f_{name}
end
";

#[test]
fn a_text_variable_names_nodes_and_fills_string_parameters() {
    let text = written(FRAMES);
    for expected in [
        "b_alpha = ",
        "b_beta = ",
        "f_alpha = frame.add@1(",
        "name=\"alpha\"",
        "name=\"beta\"",
        "output o_alpha",
        "output o_beta",
    ] {
        assert!(text.contains(expected), "{expected:?} missing in\n{text}");
    }
}

#[test]
fn a_single_name_loop_over_text_and_numbers_mixed_in_one_row_work() {
    let text = written(
        "for part in [\"left\", \"right\"]\n  p_{part} = solid.box@1(size=[1, 1, 1])\n  output out_{part} = p_{part}\nend\n",
    );
    assert!(
        text.contains("p_left") && text.contains("p_right"),
        "{text}"
    );
}

#[test]
fn dollar_braces_splice_the_text_inside_a_string() {
    let text = written(
        "for side in [\"l\", \"r\"]\n  b_{side} = solid.box@1(size=[1, 1, 1])\n  f_{side} = frame.add@1(b_{side}, name=\"hand_${side}\", position=[0, 0, 0], rotation=[0, 0, 0])\n  output o_{side} = f_{side}\nend\n",
    );
    assert!(
        text.contains("name=\"hand_l\"") && text.contains("name=\"hand_r\""),
        "{text}"
    );
}

#[test]
fn a_comma_inside_quotes_does_not_split_the_list() {
    let text = written(
        "for [id, label] in [[0, \"a,b\"], [1, \"c\"]]\n  b_{id} = solid.box@1(size=[1, 1, 1])\n  f_{id} = frame.add@1(b_{id}, name={label}, position=[0, 0, 0], rotation=[0, 0, 0])\n  output o_{id} = f_{id}\nend\n",
    );
    assert!(
        text.contains("name=\"a,b\"") && text.contains("name=\"c\""),
        "{text}"
    );
}

#[test]
fn text_that_cannot_be_part_of_a_node_name_is_refused_with_the_fix() {
    let e = error("for n in [\"a-b\"]\n  p_{n} = solid.box@1(size=[1, 1, 1])\nend\n");
    assert!(e.message.contains("cannot go inside a node name"), "{e:?}");
    assert!(
        e.message.contains("n=\"a-b\""),
        "the error names the iteration: {e:?}"
    );
    assert!(
        e.suggestion
            .as_deref()
            .unwrap_or("")
            .contains("underscores"),
        "{e:?}"
    );
    assert_eq!(e.span.unwrap().line, 2);
}

#[test]
fn quotes_or_backslashes_inside_a_loop_text_are_refused() {
    let e = error("for n in [\"a\\\\b\"]\n  p_{n} = solid.box@1(size=[1, 1, 1])\nend\n");
    assert!(e.message.contains("no quotes or backslashes"), "{e:?}");
}

#[test]
fn a_loop_variable_shadows_a_constant_of_the_same_name() {
    let text = written(
        "let k = 3\nfor k in [\"s\"]\n  b_{k} = solid.box@1(size=[1, 1, 1])\n  output o_{k} = b_{k}\nend\nc = solid.box@1(size=[{k}, 1, 1])\n",
    );
    assert!(text.contains("b_s"), "{text}");
}

#[test]
fn numeric_loops_still_work_unchanged() {
    let text = written("for i in 0..3\n  p_{i} = solid.box@1(size=[{i + 1}, 1, 1])\nend\n");
    assert!(text.contains("p_0") && text.contains("p_2"), "{text}");
}
