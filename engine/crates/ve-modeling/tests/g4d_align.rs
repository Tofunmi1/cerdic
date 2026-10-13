//! G4d alignment is independent of the pending socket ownership decisions.
#![expect(clippy::unwrap_used, reason = "test fixture construction")]
use ve_graph::{Outcome, Session, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn evaluate(
    from: &str,
    to: &str,
    pivot: &str,
) -> Result<ve_modeling::Evaluation, Box<ve_graph::Diagnostic>> {
    let source = format!(
        "b = solid.box(size=[2,4,6])\nm = xform.move(b, by=[3,0,0])\na = xform.align(m, from={from}, to={to}, pivot={pivot})\noutput a"
    );
    let parsed = parse_text(&builtin_registry(), &source).unwrap();
    let mut session = Session::with_builtin(4);
    let batch = serde_json::json!({"doc": session.document().id, "base_revision": 0,
        "transaction": "t_align", "commands": parsed.commands});
    assert!(matches!(
        session.apply_json(&batch.to_string()),
        Outcome::Committed { .. }
    ));
    Evaluator::default().evaluate(session.document(), Quality::Preview)
}

fn bounds(result: &ve_modeling::Evaluation) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for p in &result.parts[0].positions {
        for i in 0..3 {
            min[i] = min[i].min(p[i]);
            max[i] = max[i].max(p[i]);
        }
    }
    (min, max)
}

fn close(actual: [f32; 3], expected: [f32; 3]) {
    assert!(
        actual
            .into_iter()
            .zip(expected)
            .all(|(a, b)| (a - b).abs() < 1e-5)
    );
}

#[test]
fn g4d_04_alignment_uses_explicit_pivot_and_preserves_volume() {
    let result = evaluate("[1,0,0]", "[0,1,0]", "[3,0,0]").unwrap();
    let (min, max) = bounds(&result);
    close(min, [1.0, -1.0, -3.0]);
    close(max, [5.0, 1.0, 3.0]);
    assert!((result.parts[0].volume - 48.0).abs() < 1e-6);
}

#[test]
fn g4d_04_parallel_and_antiparallel_have_defined_results() {
    let (min, max) = bounds(&evaluate("[2,0,0]", "[10,0,0]", "[0,0,0]").unwrap());
    close(min, [2.0, -2.0, -3.0]);
    close(max, [4.0, 2.0, 3.0]);
    let (min, max) = bounds(&evaluate("[1,0,0]", "[-1,0,0]", "[0,0,0]").unwrap());
    close(min, [-4.0, -2.0, -3.0]);
    close(max, [-2.0, 2.0, 3.0]);
}

#[test]
fn g4d_04_zero_direction_is_typed_refusal() {
    let error = evaluate("[0,0,0]", "[0,1,0]", "[0,0,0]").unwrap_err();
    assert_eq!(error.code, "kernel.invalid_input");
    assert!(error.node.is_some());
    assert!(error.message.contains("direction"));
}

#[test]
fn g4d_04_small_angle_and_oblique_alignment_remain_proper() {
    for (from, to) in [
        ("[1,0,0]", "[1,0.00000001,0]"),
        ("[1,2,3]", "[-3,2,1]"),
        ("[1,0,0]", "[-1,0.00000001,0]"),
    ] {
        let result = evaluate(from, to, "[3,0,0]").unwrap();
        assert!((result.parts[0].volume - 48.0).abs() < 1e-6);
        let (min, max) = bounds(&result);
        for i in 0..3 {
            assert!((min[i] + max[i] - if i == 0 { 6.0 } else { 0.0 }).abs() < 1e-5);
        }
    }
}
