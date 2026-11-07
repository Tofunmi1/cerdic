//! `let name = join(i in 0..8, "template", "separator")` builds a text constant with a loop, for the lists and strings a model would otherwise write
//! by hand: `points=[${pts}]`, `keys="${keys}"`. `${name}` splices raw text inside and outside a string. Written before the body; do not weaken.
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
        transaction: TransactionId::parse("t_join").unwrap(),
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

#[test]
fn a_joined_string_fills_the_keys_of_a_track() {
    let text = written(
        "let keys = join(i in 0..3, \"{i * 0.5}:0,{i * 10},0\", \"; \")\nb = solid.box@1(size=[1, 1, 1])\nt = anim.track@1(b, property=\"rotation\", keys=\"${keys}\")\noutput o = t\n",
    );
    assert!(text.contains("0:0,0,0; 0.5:0,10,0; 1:0,20,0"), "{text}");
}

#[test]
fn a_joined_list_fills_the_points_of_a_curve_outside_a_string() {
    let text = written(
        "let pts = join(i in 0..3, \"[{i}, 0, 0]\")\nc = curve.polyline@1(points=[${pts}])\noutput o = c\n",
    );
    for point in ["[0, 0, 0]", "[1, 0, 0]", "[2, 0, 0]"] {
        assert!(text.contains(point), "{point} missing in {text}");
    }
}

#[test]
fn rows_may_bind_text_and_numbers_and_the_separator_is_chosen() {
    let text = written(
        "let tag = join([name, x] in [[\"a\", 1], [\"b\", 2]], \"${name}:{x}\", \"|\")\nb = solid.box@1(size=[1, 1, 1])\nf = frame.add@1(b, name=\"${tag}\", position=[0, 0, 0], rotation=[0, 0, 0])\noutput o = f\n",
    );
    assert!(text.contains("name=\"a:1|b:2\""), "{text}");
}

#[test]
fn a_join_inside_a_loop_sees_the_loop_variable() {
    let text = written(
        "for k in 0..2\n  let ks = join(i in 0..2, \"{i + k}:0,0,0\", \"; \")\n  b_{k} = solid.box@1(size=[1, 1, 1])\n  t_{k} = anim.track@1(b_{k}, property=\"rotation\", keys=\"${ks}\")\n  output o_{k} = t_{k}\nend\n",
    );
    assert!(text.contains("0:0,0,0; 1:0,0,0"), "{text}");
    assert!(text.contains("1:0,0,0; 2:0,0,0"), "{text}");
}

#[test]
fn a_hash_inside_a_template_is_not_a_comment() {
    let text = written(
        "let colours = join(i in 0..2, \"{i}:#ff0000\", \", \")\nb = solid.box@1(size=[1, 1, 1])\nf = frame.add@1(b, name=\"${colours}\", position=[0, 0, 0], rotation=[0, 0, 0])\noutput o = f\n",
    );
    assert!(text.contains("0:#ff0000, 1:#ff0000"), "{text}");
}

#[test]
fn a_bad_expression_in_the_template_names_the_iteration() {
    let e = error("let k = join(i in 0..3, \"{1 / (i - 1)}\")\nb = solid.box@1(size=[1, 1, 1])\n");
    assert!(e.message.contains("division by zero"), "{e:?}");
    assert!(e.message.contains("in iteration i=1"), "{e:?}");
}

#[test]
fn a_join_without_a_quoted_template_or_with_extra_arguments_is_explained() {
    let e = error("let k = join(i in 0..3, i)\nb = solid.box@1(size=[1, 1, 1])\n");
    assert!(
        e.message.contains("template of a join is a quoted text"),
        "{e:?}"
    );
    let e =
        error("let k = join(i in 0..3, \"{i}\", \", \", \"x\")\nb = solid.box@1(size=[1, 1, 1])\n");
    assert!(e.message.contains("a join is written"), "{e:?}");
    let e = error("let k = join(i in 0..3, \"{i}\", 5)\nb = solid.box@1(size=[1, 1, 1])\n");
    assert!(
        e.message.contains("separator of a join is a quoted text"),
        "{e:?}"
    );
}

#[test]
fn a_join_that_grows_past_the_limit_is_refused() {
    let e = error(
        "let k = join(i in 0..2000, \"abcdefghijklmnopqrstuvwxyz0123456789abcdefghijklmnopqrstuvwxyz0123456789\")\nb = solid.box@1(size=[1, 1, 1])\n",
    );
    assert!(e.message.contains("longer than"), "{e:?}");
}

#[test]
fn an_unknown_constant_in_a_template_is_named() {
    let e = error("let k = join(i in 0..2, \"${nothing}\")\nb = solid.box@1(size=[1, 1, 1])\n");
    assert!(e.message.contains("nothing"), "{e:?}");
}

#[test]
fn a_constant_cannot_be_defined_twice_by_join() {
    let e = error(
        "let k = join(i in 0..2, \"{i}\")\nlet k = join(i in 0..2, \"{i}\")\nb = solid.box@1(size=[1, 1, 1])\n",
    );
    assert!(e.message.contains("already defined"), "{e:?}");
}
