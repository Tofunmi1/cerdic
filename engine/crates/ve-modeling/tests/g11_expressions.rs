//! Constants and `{expression}` values in the text language, from the program a model writes to the evaluated solid.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "the helper unwraps fixtures that are known to be valid and reports failures by panicking"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn volume(text: &str) -> f64 {
    let batch = parse_text(&builtin_registry(), text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g11_expressions").unwrap(),
        label: None,
        commands: batch.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap_or_else(|e| panic!("{}: {}", e.code, e.message))
        .parts[0]
        .volume
}

#[test]
fn constants_give_the_same_solid_as_the_numbers_written_out() {
    let with = volume(
        "let w = 1.5\nlet d = w / 2\nbox = solid.box@1(size=[{w}, {w * 2}, {d}])\noutput box",
    );
    let without = volume("box = solid.box@1(size=[1.5, 3, 0.75])\noutput box");
    assert!(
        (with - without).abs() < 1.0e-9,
        "with {with}, without {without}"
    );
    assert!((with - 3.375).abs() < 1.0e-6, "volume {with}");
}

#[test]
fn a_unit_after_the_brace_scales_the_value() {
    // 1500 mm is 1.5 m; the box is 1.5 x 1 x 1.
    let v = volume("let w = 1.5\nbox = solid.box@1(size=[{w * 1000}mm, 1, 1])\noutput box");
    assert!((v - 1.5).abs() < 1.0e-6, "volume {v}");
}

#[test]
fn one_constant_drives_a_whole_family_of_parts() {
    let program = |w: &str| {
        format!(
            "let w = {w}\n\
             a = solid.box@1(size=[{{w}}, {{w}}, {{w}}])\n\
             b = solid.box@1(size=[{{w / 2}}, {{w / 2}}, {{w / 2}}])\n\
             moved = xform.move@1(b, by=[{{w}}, 0, 0])\n\
             both = solid.union@1(a, moved)\n\
             output both"
        )
    };
    let (small, big) = (volume(&program("1")), volume(&program("2")));
    // Two cubes, one of edge w and one of edge w/2, so the volume scales with the cube of w (1.125 for w = 1).
    assert!((small - 1.125).abs() < 1.0e-6, "w=1: {small}");
    assert!((big - 8.0 * small).abs() < 1.0e-5, "w=2: {big}");
}

#[test]
fn a_mistake_is_reported_at_the_expression_with_a_fix() {
    let errors = parse_text(
        &builtin_registry(),
        "box = solid.box@1(size=[{width}, 1, 1])\noutput box",
    )
    .unwrap_err();
    let first = &errors[0];
    assert_eq!(first.code, "parse.expression");
    assert!(first.message.contains("width is not defined"));
    assert!(
        first
            .suggestion
            .as_deref()
            .is_some_and(|s| s.contains("let name")),
        "{:?}",
        first.suggestion
    );
    assert_eq!(first.span.map(|s| s.line), Some(1));
}
