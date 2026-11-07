//! `def name(a, b) ... end` and `name(1, "x")` in the text language: a block written once and read for each call with its parameters bound to the arguments
//! (numbers, expressions, text). The body may hold nodes, `let`, `for`, `if` and calls of other macros, and sees the constants above the call. A macro
//! never calls itself, a `def` stands at the top level, and the 2,000-statement cap still counts what calls produce. Written before the body; do not weaken.
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
        transaction: TransactionId::parse("t_macros").unwrap(),
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

const MARK: &str = "\
def mark(name, x, y)
  b_{name} = solid.box@1(size=[0.2, 0.2, 0.2])
  m_{name} = xform.move@1(b_{name}, by=[{x}, {y}, 0])
  f_{name} = frame.add@1(m_{name}, name={name}, position=[0, 0, 0], rotation=[0, 0, 0])
  output {name} = f_{name}
end
";

#[test]
fn a_call_reads_the_body_with_its_arguments() {
    let text = written(&format!("{MARK}mark(\"alpha\", 1, 2)\n"));
    for expected in [
        "b_alpha = ",
        "m_alpha = ",
        "f_alpha = frame.add@1(",
        "name=\"alpha\"",
        "output alpha",
    ] {
        assert!(text.contains(expected), "{expected:?} missing in\n{text}");
    }
    assert!(text.contains("by=[1, 2, 0]"), "{text}");
}

#[test]
fn each_call_makes_its_own_nodes() {
    let text = written(&format!(
        "{MARK}mark(\"a\", 0, 0)\nmark(\"b\", 1, 0)\nmark(\"c\", 2, 0)\n"
    ));
    for name in [
        "b_a = ", "b_b = ", "b_c = ", "output a", "output b", "output c",
    ] {
        assert!(text.contains(name), "{name} missing in {text}");
    }
}

#[test]
fn arguments_may_be_expressions_braces_and_constants() {
    let text = written(&format!(
        "let k = 3\nlet who = \"zed\"\n{MARK}mark(who, k * 2, {{k + 1}})\n"
    ));
    assert!(
        text.contains("name=\"zed\"") && text.contains("by=[6, 4, 0]"),
        "{text}"
    );
}

#[test]
fn the_body_sees_constants_above_the_call_and_a_parameter_shadows_one() {
    let text = written(
        "let size = 2\ndef cube(size)\n  c_{size} = solid.box@1(size=[{size}, 1, 1])\nend\ndef wide()\n  w = solid.box@1(size=[{size}, 1, 1])\nend\ncube(5)\nwide()\n",
    );
    assert!(text.contains("c_5 = ") && text.contains("w = "), "{text}");
    assert!(
        text.contains("size=[5, 1, 1]") && text.contains("size=[2, 1, 1]"),
        "{text}"
    );
}

#[test]
fn loops_and_ifs_work_inside_a_macro() {
    let text = written(
        "def row(tag, n)\n  for i in 0..{n}\n    if i < 2\n      lo_{tag}_{i} = solid.box@1(size=[1, 1, 1])\n    else\n      hi_{tag}_{i} = solid.box@1(size=[1, 1, 1])\n    end\n  end\nend\nrow(\"r\", 4)\n",
    );
    for name in ["lo_r_0 = ", "lo_r_1 = ", "hi_r_2 = ", "hi_r_3 = "] {
        assert!(text.contains(name), "{name} missing in {text}");
    }
}

#[test]
fn a_macro_may_call_another_macro() {
    let text = written(&format!(
        "{MARK}def pair(a, b)\n  mark(a, 0, 0)\n  mark(b, 1, 0)\nend\npair(\"p\", \"q\")\n"
    ));
    assert!(text.contains("b_p = ") && text.contains("b_q = "), "{text}");
}

#[test]
fn a_call_inside_a_loop_gets_the_loop_variable() {
    let text = written(&format!(
        "{MARK}for i in 0..3\n  mark(\"n${{i}}\", i, 0)\nend\n"
    ));
    for name in ["b_n0 = ", "b_n1 = ", "b_n2 = ", "output n0", "output n2"] {
        assert!(text.contains(name), "{name} missing in {text}");
    }
    assert!(text.contains("by=[2, 0, 0]"), "{text}");
}

#[test]
fn braces_inside_quotes_are_not_evaluated_so_a_name_with_them_is_refused() {
    let e = error(&format!(
        "{MARK}for i in 0..2\n  mark(\"n{{i}}\", i, 0)\nend\n"
    ));
    assert!(e.message.contains("cannot go inside a node name"), "{e:?}");
}

#[test]
fn a_macro_cannot_call_itself_directly_or_through_another() {
    let e = error("def a()\n  a()\nend\na()\n");
    assert!(e.message.contains("cannot call itself"), "{e:?}");
    let e = error("def a()\n  b()\nend\ndef b()\n  a()\nend\na()\n");
    assert!(e.message.contains("cannot call itself"), "{e:?}");
}

#[test]
fn the_wrong_number_of_arguments_is_explained() {
    let e = error(&format!("{MARK}mark(\"a\", 1)\n"));
    assert!(
        e.message
            .contains("mark takes 3 argument(s) (name, x, y), got 2"),
        "{e:?}"
    );
}

#[test]
fn a_def_must_be_at_the_top_level_once_and_hold_only_what_a_macro_may() {
    let e = error("for i in 0..2\n  def a()\n    x = solid.box@1(size=[1, 1, 1])\n  end\nend\n");
    assert!(e.message.contains("top level"), "{e:?}");
    let e = error(
        "def a()\n  x = solid.box@1(size=[1, 1, 1])\nend\ndef a()\n  y = solid.box@1(size=[1, 1, 1])\nend\n",
    );
    assert!(e.message.contains("already defined"), "{e:?}");
    let e = error(
        "def a()\n  group g\n    s = solid.box@1(size=[1, 1, 1])\n    result s\n  end\nend\n",
    );
    assert!(
        e.message
            .contains("not allowed inside a loop, an if or a macro"),
        "{e:?}"
    );
    let e = error("def a()\n  def b()\n  end\nend\n");
    assert!(e.message.contains("top level"), "{e:?}");
    let e = error("def a(x, x)\n  q = solid.box@1(size=[1, 1, 1])\nend\n");
    assert!(e.message.contains("cannot name a parameter"), "{e:?}");
}

#[test]
fn an_error_inside_a_body_names_the_macro_its_arguments_and_the_body_line() {
    let e =
        error("def a(n)\n  ok = solid.box@1(size=[1, 1, 1])\n  bad = missing.op()\nend\na(7)\n");
    assert_eq!(e.span.unwrap().line, 3, "{e:?}");
    assert!(
        e.message.contains("macro=a") && e.message.contains("n=7"),
        "{e:?}"
    );
}

#[test]
fn the_statement_cap_counts_what_calls_produce() {
    let e = error(
        "def three(i)\n  a_{i} = solid.box@1(size=[1, 1, 1])\n  b_{i} = solid.box@1(size=[1, 1, 1])\n  c_{i} = solid.box@1(size=[1, 1, 1])\nend\nfor i in 0..1000\n  three(i)\nend\n",
    );
    assert_eq!(e.code, "parse.too_big", "{e:?}");
}

#[test]
fn a_macro_may_be_called_before_nothing_else_is_named_like_it() {
    let text = written(
        "def box2()\n  x = solid.box@1(size=[1, 1, 1])\nend\nbox2()\ny = solid.box@1(size=[2, 1, 1])\n",
    );
    assert!(text.contains("x = ") && text.contains("y = "), "{text}");
}
