//! G11: loop cut a closed quad ring with fixed geometry and refusal checks.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "fixture failures report their evaluation diagnostic"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

const MESH: &str = "box = solid.box@1(size=[1,1,1])\ntris = poly.from_solid@1(box)\nmesh = poly.merge_coplanar@1(tris)\n";
fn evaluate(text: &str) -> Result<Evaluation, (String, Option<String>)> {
    let batch = parse_text(&builtin_registry(), text).map_err(|errors| {
        let error = &errors[0];
        (error.code.clone(), error.suggestion.clone())
    })?;
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g11_loopcut").unwrap(),
        label: None,
        commands: batch.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.suggestion.clone()))
}
fn run(text: &str) -> Evaluation {
    evaluate(text).unwrap_or_else(|(code, suggestion)| panic!("{code}: {suggestion:?}"))
}
fn bounds(result: &Evaluation, axis: usize) -> [f32; 2] {
    result.parts[0]
        .positions
        .iter()
        .fold([f32::INFINITY, f32::NEG_INFINITY], |mut b, p| {
            b[0] = b[0].min(p[axis]);
            b[1] = b[1].max(p[axis]);
            b
        })
}
fn cut(cuts: usize) -> String {
    format!("{MESH}cut = poly.loop_cut@1(mesh, at=[0.5,0,0.5], cuts={cuts})\noutput cut")
}

#[test]
fn one_cut_adds_four_points() {
    let result = run(&cut(1));
    assert_eq!(result.parts[0].positions.len(), 12);
}

#[test]
fn three_cuts_add_twelve_points() {
    let result = run(&cut(3));
    assert_eq!(result.parts[0].positions.len(), 20);
}

#[test]
fn volume_and_bounds_are_unchanged_for_one_and_three_cuts() {
    for cuts in [1, 3] {
        let result = run(&format!(
            "{MESH}cut = poly.loop_cut@1(mesh, at=[0.5,0,0.5], cuts={cuts})\nsolid = poly.to_solid@1(cut)\noutput solid"
        ));
        assert!(
            (result.parts[0].volume - 1.0).abs() < 1e-6,
            "cuts={cuts}, volume={}",
            result.parts[0].volume
        );
        for axis in 0..3 {
            let measured = bounds(&result, axis);
            assert!(
                (measured[0] + 0.5).abs() < 1e-6 && (measured[1] - 0.5).abs() < 1e-6,
                "cuts={cuts}, axis={axis}, bounds={measured:?}"
            );
        }
    }
}

#[test]
fn three_cuts_are_evenly_spaced() {
    let result = run(&cut(3));
    let mut ys: Vec<_> = result.parts[0].positions.iter().map(|p| p[1]).collect();
    ys.sort_by(f32::total_cmp);
    ys.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    assert_eq!(ys.len(), 5);
    for (actual, expected) in ys.iter().zip([-0.5, -0.25, 0.0, 0.25, 0.5]) {
        assert!((actual - expected).abs() < 1e-6, "y values {ys:?}");
    }
}

#[test]
fn at_selects_the_other_closed_quad_ring() {
    let result = run(&format!(
        "{MESH}cut = poly.loop_cut@1(mesh, at=[0.5,0.5,0])\noutput cut"
    ));
    assert_eq!(result.parts[0].positions.len(), 12);
    let count = result.parts[0]
        .positions
        .iter()
        .filter(|p| p[0].abs() < 1e-6 || p[2].abs() < 1e-6)
        .count();
    assert_eq!(count, 4, "new points on a crossed axis: {count}");
}

#[test]
fn triangle_ring_refuses_with_merge_coplanar_suggestion() {
    let (code, suggestion) = evaluate("box = solid.box@1(size=[1,1,1])\ntris = poly.from_solid@1(box)\ncut = poly.loop_cut@1(tris, at=[0.5,0,0.5])\noutput cut").unwrap_err();
    assert!(code.starts_with("poly."), "code {code}");
    assert!(
        suggestion
            .as_ref()
            .is_some_and(|s| s.contains("poly.merge_coplanar")),
        "suggestion {suggestion:?}"
    );
}

#[test]
fn cut_count_range_is_typed() {
    for cuts in [0, 33] {
        let errors = parse_text(&builtin_registry(), &cut(cuts)).unwrap_err();
        assert_eq!(errors[0].code, "param.range", "cuts={cuts}");
        assert!(
            errors[0].message.contains("1 to 32"),
            "range: {}",
            errors[0].message
        );
    }
}

#[test]
fn repeated_evaluations_have_bit_identical_positions() {
    assert_eq!(
        run(&cut(1)).parts[0].positions,
        run(&cut(1)).parts[0].positions
    );
}
