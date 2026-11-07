//! `curve.expr`, `poly.surface_expr` and `inst.program`: curves, surfaces and copies written as programs. Written before the body; do not weaken.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "fixture setup unwraps data that is known to be valid"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, (String, String)> {
    let commands = parse_text(&builtin_registry(), source)
        .unwrap_or_else(|e| panic!("{e:?}"))
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_generators").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message.clone()))
}

fn bounds(result: &Evaluation) -> ([f32; 3], [f32; 3]) {
    let (mut low, mut high) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for part in &result.parts {
        for p in &part.positions {
            for axis in 0..3 {
                low[axis] = low[axis].min(p[axis]);
                high[axis] = high[axis].max(p[axis]);
            }
        }
    }
    (low, high)
}

/// Where the copies of a placed set stand: the translation of each transform, over all parts.
fn copy_centres(result: &Evaluation) -> Vec<[f32; 3]> {
    result
        .parts
        .iter()
        .flat_map(|part| part.transforms.iter().flatten())
        .map(|m| [m[12], m[13], m[14]])
        .collect()
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

#[test]
fn a_curve_program_draws_a_helix_the_tube_can_follow() {
    let result = evaluate(
        "c = curve.expr@1(samples=120, code=\"out = vec3(cos(t * 18.85), t * 2.0, sin(t * 18.85))\")\ntube = curve.tube@1(c, radius=0.02, sides=6)\noutput o = tube\n",
    )
    .unwrap();
    let (low, high) = bounds(&result);
    assert!(
        near(low[1], -0.02, 0.03) && near(high[1], 2.02, 0.03),
        "{low:?} {high:?}"
    );
    assert!(
        near(high[0], 1.0, 0.06) && near(low[0], -1.0, 0.06),
        "{low:?} {high:?}"
    );
}

#[test]
fn a_closed_curve_program_does_not_repeat_its_first_point() {
    let result = evaluate(
        "c = curve.expr@1(samples=48, closed=true, code=\"out = vec3(cos(t * 6.2832), 0.0, sin(t * 6.2832))\")\nring = curve.tube@1(c, radius=0.05, sides=6)\noutput o = ring\n",
    )
    .unwrap();
    let (low, high) = bounds(&result);
    assert!(
        near(high[0] - low[0], 2.1, 0.1),
        "a ring about 2 m across: {low:?} {high:?}"
    );
}

#[test]
fn a_curve_that_stands_still_is_refused_with_the_fix() {
    let error = evaluate("c = curve.expr@1(samples=8, code=\"out = vec3(1.0, 2.0, 3.0)\")\ntube = curve.tube@1(c, radius=0.1)\noutput o = tube\n")
        .err()
        .unwrap();
    assert_eq!(error.0, "geometry.degenerate_curve");
}

#[test]
fn a_surface_program_makes_a_torus() {
    let result = evaluate(
        "s = poly.surface_expr@1(columns=48, rows=24, code=\"r = 1.0 + 0.3 * cos(v * 6.2832); out = vec3(r * cos(u * 6.2832), 0.3 * sin(v * 6.2832), r * sin(u * 6.2832))\")\noutput o = s\n",
    )
    .unwrap();
    let (low, high) = bounds(&result);
    assert!(
        near(high[0], 1.3, 0.03) && near(low[0], -1.3, 0.03),
        "{low:?} {high:?}"
    );
    assert!(
        near(high[1], 0.3, 0.02) && near(low[1], -0.3, 0.02),
        "{low:?} {high:?}"
    );
}

#[test]
fn a_surface_program_feeds_the_vertex_program() {
    let result = evaluate(
        "s = poly.surface_expr@1(columns=16, rows=16, code=\"out = vec3(u * 4.0, 0.0, v * 4.0)\")\nm = poly.program@1(s, mode=\"offset\", code=\"out = vec3(0.0, 0.5, 0.0)\")\noutput o = m\n",
    )
    .unwrap();
    let (low, high) = bounds(&result);
    assert!(
        near(low[1], 0.5, 1e-4) && near(high[1], 0.5, 1e-4) && near(high[0], 4.0, 1e-4),
        "{low:?} {high:?}"
    );
}

/// The y component of the summed triangle normals of the first part (positive: the surface faces up).
fn facing_y(result: &Evaluation) -> f32 {
    let part = &result.parts[0];
    part.indices
        .chunks_exact(3)
        .map(|t| {
            let (a, b, c) = (
                part.positions[t[0] as usize],
                part.positions[t[1] as usize],
                part.positions[t[2] as usize],
            );
            let (e, f) = ([b[0] - a[0], b[2] - a[2]], [c[0] - a[0], c[2] - a[2]]);
            // y of (b - a) x (c - a)
            e[1] * f[0] - e[0] * f[1]
        })
        .sum()
}

#[test]
fn flip_reverses_which_side_of_a_surface_is_the_front() {
    let source = |flip: bool| {
        format!(
            "s = poly.surface_expr@1(columns=4, rows=4, flip={flip}, code=\"out = vec3(u, 0.0, v)\")\noutput o = s\n"
        )
    };
    let plain = facing_y(&evaluate(&source(false)).unwrap());
    let flipped = facing_y(&evaluate(&source(true)).unwrap());
    assert!(plain.abs() > 0.1, "{plain}");
    assert!(
        plain * flipped < 0.0,
        "{plain} then {flipped}: flip turns it over"
    );
}

#[test]
fn copies_can_follow_a_spiral_and_shrink() {
    let result = evaluate(
        "b = solid.box@1(size=[0.2, 0.2, 0.2])\ncopies = inst.program@1(b, count=20, code=\"out = vec3(cos(i * 0.5) * (1.0 + f * 5.0), 0.0, sin(i * 0.5) * (1.0 + f * 5.0))\", scale_code=\"out = 1.0 - 0.5 * f\")\noutput o = copies\n",
    )
    .unwrap();
    let centres = copy_centres(&result);
    assert_eq!(centres.len(), 20);
    let reach = |c: &[f32; 3]| (c[0] * c[0] + c[2] * c[2]).sqrt();
    assert!(near(reach(&centres[0]), 1.0, 1e-3), "{:?}", centres[0]);
    assert!(near(reach(&centres[19]), 6.0, 1e-3), "{:?}", centres[19]);
}

#[test]
fn copy_programs_see_the_count_and_the_fraction() {
    let result = evaluate(
        "b = solid.box@1(size=[0.1, 0.1, 0.1])\ncopies = inst.program@1(b, count=5, code=\"out = vec3(f * 4.0, count, 0.0)\")\noutput o = copies\n",
    )
    .unwrap();
    let centres = copy_centres(&result);
    assert_eq!(centres.len(), 5);
    assert!(
        near(centres[0][0], 0.0, 1e-4) && near(centres[4][0], 4.0, 1e-4),
        "{centres:?}"
    );
    assert!(
        centres.iter().all(|c| near(c[1], 5.0, 1e-4)),
        "count is 5: {centres:?}"
    );
}

#[test]
fn a_non_positive_size_is_refused_naming_the_copy() {
    let error = evaluate(
        "b = solid.box@1(size=[0.1, 0.1, 0.1])\ncopies = inst.program@1(b, count=4, scale_code=\"out = 1.0 - i\")\noutput o = copies\n",
    )
    .err()
    .unwrap();
    assert_eq!(error.0, "expr.error");
    assert!(error.1.contains("copy 1"), "{}", error.1);
}

#[test]
fn a_mistake_in_a_generator_program_names_line_and_column() {
    let error = evaluate("c = curve.expr@1(samples=8, code=\"out = vec3(1.0,\")\ntube = curve.tube@1(c, radius=0.1)\noutput o = tube\n")
        .err()
        .unwrap();
    assert_eq!(error.0, "expr.error");
    assert!(error.1.contains("line 1"), "{}", error.1);
}
