//! G9a-1: boolean chains should be estimated by bounded growth and still enforce real budgets.
#![expect(
    clippy::expect_used,
    reason = "fixture setup reports failures with context"
)]
use std::fmt::Write as _;
use ve_graph::{
    CommandBatch, Diagnostic, Outcome, Session, TransactionId, builtin_registry, parse_text,
};
use ve_modeling::{Evaluator, Quality};

fn evaluate(text: &str) -> Result<ve_modeling::Evaluation, Box<Diagnostic>> {
    let batch = parse_text(&builtin_registry(), text).expect("fixture text parses");
    let mut session = Session::with_builtin(9);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g9_a1").expect("valid transaction id"),
        label: None,
        commands: batch.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    Evaluator::default().evaluate(session.document(), Quality::Preview)
}

#[test]
fn g9_a1_eight_chained_sphere_cuts_fit_preview() {
    let mut text = String::from("base = solid.box@1(size=[1, 1, 1])\n");
    let mut previous = String::from("base");
    for index in 0..8 {
        let x = -0.35 + f64::from(index) * 0.1;
        let sphere = format!("cutter_{index}");
        let moved = format!("moved_{index}");
        let cut = format!("cut_{index}");
        let _ = writeln!(text, "{sphere} = solid.sphere@1(radius=0.07, segments=16)");
        let _ = writeln!(text, "{moved} = xform.move@1({sphere}, by=[{x}, 0.5, 0])");
        let _ = writeln!(text, "{cut} = solid.difference@1({previous}, {moved})");
        previous = cut;
    }
    let _ = writeln!(text, "output {previous}");
    let result = evaluate(&text).expect("eight small cuts are within Preview budget");
    assert_eq!(result.parts.len(), 1);
    assert!(result.parts[0].volume > 0.0);
}

#[test]
fn g9_a1_large_array_is_still_refused_by_preflight() {
    let result = evaluate(
        "sphere = solid.sphere@1(radius=0.5, segments=100)\n\
         copies = xform.array@1(sphere, count_x=32, count_y=32, count_z=1, spacing=[2, 2, 0])\n\
         output copies\n",
    );
    let failure = result.expect_err("array output exceeds the Preview triangle budget");
    assert_eq!(failure.code, "budget.triangles");
}

#[test]
fn g9_a1_deep_subdivision_of_a_dense_cage_is_refused_by_preflight() {
    let result = evaluate(
        "sphere = solid.sphere@1(radius=0.5, segments=100)\n\
         cage = poly.from_solid@1(sphere)\n\
         refined = poly.subdivide@1(cage, levels=3)\n\
         output refined\n",
    );
    let failure = result.expect_err("deep refinement exceeds the Preview triangle budget");
    assert_eq!(failure.code, "budget.triangles");
}
