//! `if condition ... else ... end` in the text language: only the branch that holds is read. A condition compares numbers (`i < 3`) or text (`side == "l"`),
//! joins with `&&` and `||`, negates with `!`, and may use parentheses; a lone number is true when it is not zero. A `let` inside a branch stays inside it,
//! errors keep their source line, and a node may still be named `if`. Written before the body; do not weaken.
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
        transaction: TransactionId::parse("t_if").unwrap(),
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

const BOX: &str = "solid.box@1(size=[1, 1, 1])";

#[test]
fn the_branch_that_holds_is_read_and_the_other_is_not() {
    let text = written(&format!(
        "let big = 1\nif big\n  yes = {BOX}\nelse\n  no = {BOX}\nend\n"
    ));
    assert!(text.contains("yes = ") && !text.contains("no = "), "{text}");
    let text = written(&format!(
        "let big = 0\nif big\n  yes = {BOX}\nelse\n  no = {BOX}\nend\n"
    ));
    assert!(text.contains("no = ") && !text.contains("yes = "), "{text}");
}

#[test]
fn an_if_without_an_else_reads_nothing_when_it_does_not_hold() {
    let text = written(&format!("keep = {BOX}\nif 1 > 2\n  gone = {BOX}\nend\n"));
    assert!(
        text.contains("keep = ") && !text.contains("gone = "),
        "{text}"
    );
}

#[test]
fn comparisons_of_numbers_work_in_every_form() {
    for (condition, holds) in [
        ("1 < 2", true),
        ("2 < 2", false),
        ("2 <= 2", true),
        ("3 > 2", true),
        ("2 >= 3", false),
        ("2 == 2", true),
        ("2 != 2", false),
        ("0.1 + 0.2 == 0.3", true),
        ("2 * 3 > 5", true),
        ("0", false),
        ("-1", true),
    ] {
        let text = written(&format!(
            "if {condition}\n  a = {BOX}\nelse\n  b = {BOX}\nend\n"
        ));
        assert_eq!(text.contains("a = "), holds, "{condition}: {text}");
        assert_eq!(text.contains("b = "), !holds, "{condition}: {text}");
    }
}

#[test]
fn and_or_not_and_parentheses_combine_conditions() {
    for (condition, holds) in [
        ("1 < 2 && 2 < 3", true),
        ("1 < 2 && 3 < 2", false),
        ("3 < 2 || 2 < 3", true),
        ("3 < 2 || 4 < 3", false),
        ("!(1 < 2)", false),
        ("!0", true),
        ("(1 < 2 || 5 < 4) && 2 < 3", true),
        ("1 < 2 || 5 < 4 && 0", true),
        ("(1 < 2 || 5 < 4) && 0", false),
    ] {
        let text = written(&format!(
            "if {condition}\n  a = {BOX}\nelse\n  b = {BOX}\nend\n"
        ));
        assert_eq!(text.contains("a = "), holds, "{condition}: {text}");
    }
}

#[test]
fn inside_a_loop_the_condition_sees_the_loop_variable() {
    let text = written(&format!(
        "for i in 0..4\n  if i < 2\n    a_{{i}} = {BOX}\n  else\n    b_{{i}} = {BOX}\n  end\nend\n"
    ));
    for name in ["a_0 = ", "a_1 = ", "b_2 = ", "b_3 = "] {
        assert!(text.contains(name), "{name} missing in {text}");
    }
    assert!(
        !text.contains("a_2 = ") && !text.contains("b_0 = "),
        "{text}"
    );
}

#[test]
fn text_is_compared_with_text() {
    let text = written(&format!(
        "for [side, x] in [[\"l\", 1], [\"r\", 2]]\n  if side == \"l\"\n    p_{{side}} = solid.box@1(size=[{{x}}, 1, 1])\n  else\n    q_{{side}} = {BOX}\n  end\nend\n"
    ));
    assert!(text.contains("p_l = ") && text.contains("q_r = "), "{text}");
    assert!(
        !text.contains("p_r = ") && !text.contains("q_l = "),
        "{text}"
    );
    let text = written(&format!(
        "for side in [\"l\", \"r\"]\n  if side != \"l\"\n    only_{{side}} = {BOX}\n  end\nend\n"
    ));
    assert!(
        text.contains("only_r = ") && !text.contains("only_l = "),
        "{text}"
    );
}

#[test]
fn non_ascii_text_compares_without_trouble() {
    let text = written(&format!(
        "for side in [\"é\"]\n  if side == \"é\"\n    hit = {BOX}\n  end\nend\n"
    ));
    assert!(text.contains("hit = "), "{text}");
}

#[test]
fn ifs_nest_inside_ifs_and_inside_loops_inside_ifs() {
    let text = written(&format!(
        "let a = 1\nlet b = 0\nif a\n  if b\n    one = {BOX}\n  else\n    two = {BOX}\n  end\n  for i in 0..2\n    three_{{i}} = {BOX}\n  end\nelse\n  four = {BOX}\nend\n"
    ));
    assert!(
        text.contains("two = ") && text.contains("three_0") && text.contains("three_1"),
        "{text}"
    );
    assert!(
        !text.contains("one = ") && !text.contains("four = "),
        "{text}"
    );
}

#[test]
fn a_let_inside_a_branch_stays_inside_it() {
    let text = written(&format!(
        "if 1\n  let k = 3\n  a = solid.box@1(size=[{{k}}, 1, 1])\nend\nlet k = 5\nb = solid.box@1(size=[{{k}}, 1, 1])\n"
    ));
    assert!(text.contains("a = ") && text.contains("b = "), "{text}");
    let e = error(&format!(
        "if 1\n  let k = 3\nend\nb = solid.box@1(size=[{{k}}, 1, 1])\n"
    ));
    assert!(e.message.contains('k'), "{e:?}");
}

#[test]
fn a_node_can_still_be_named_if_or_else() {
    let text = written(&format!("if = {BOX}\nelse = {BOX}\n"));
    assert!(text.contains("if = ") && text.contains("else = "), "{text}");
}

#[test]
fn errors_after_an_if_keep_their_source_line() {
    let e = error(&format!(
        "if 1\n  a = {BOX}\nelse\n  b = {BOX}\nend\nbad = missing.op()\n"
    ));
    assert_eq!(e.span.unwrap().line, 6, "{e:?}");
    let e = error(&format!("if 1\n  a = {BOX}\n  bad = missing.op()\nend\n"));
    assert_eq!(e.span.unwrap().line, 3, "{e:?}");
}

#[test]
fn malformed_ifs_are_explained() {
    let e = error(&format!("if 1\n  a = {BOX}\n"));
    assert!(e.message.contains("missing its end"), "{e:?}");
    let e = error(&format!(
        "if 1\n  a = {BOX}\nelse\n  b = {BOX}\nelse\n  c = {BOX}\nend\n"
    ));
    assert!(e.message.contains("only one else"), "{e:?}");
    let e =
        error("if 1\n  group g\n    s = solid.box@1(size=[1, 1, 1])\n    result s\n  end\nend\n");
    assert!(
        e.message
            .contains("not allowed inside a loop, an if or a macro"),
        "{e:?}"
    );
    let e = error("if\n  a = solid.box@1(size=[1, 1, 1])\nend\n");
    assert!(
        e.message.contains("empty") || e.message.contains("expected"),
        "{e:?}"
    );
}

#[test]
fn mixing_text_and_numbers_or_ordering_text_is_refused_with_the_fix() {
    let e =
        error("for s in [\"l\"]\n  if s == 1\n    a = solid.box@1(size=[1, 1, 1])\n  end\nend\n");
    assert!(e.message.contains("compared with another text"), "{e:?}");
    let e = error(
        "for s in [\"l\"]\n  if s < \"r\"\n    a = solid.box@1(size=[1, 1, 1])\n  end\nend\n",
    );
    assert!(e.message.contains("cannot be compared with <"), "{e:?}");
    assert!(
        e.suggestion.as_deref().unwrap_or("").contains("=="),
        "{e:?}"
    );
}
