//! Variation from patterns (ADR 0060): `inst.vary` gives every placement its own scale, turn and lift from patterns (functions of
//! position), so a model can say "bigger on the right", "turned at random", "raised on the hill" without a parameter per effect.
//! Written before the bodies; do not weaken.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup unwraps data that is known to be valid"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| format!("{e:?}"))?
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g15_vary").unwrap(),
        label: None,
        commands,
    });
    if !matches!(applied, Outcome::Committed { .. }) {
        return Err(format!("{applied:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| format!("{e:?}"))
}

/// Every placement matrix of the first part (column-major, translation in 12..15).
fn matrices(source: &str) -> Vec<[f32; 16]> {
    let result = evaluate(source).unwrap();
    let flat = result.parts[0].transforms.as_ref().unwrap();
    flat.iter().copied().collect()
}

const ROW: &str = "b = solid.box@1(size=[1, 1, 1])\ns = inst.array@1(b, count_x=4, count_y=1, count_z=1, spacing=[40, 0, 0])\n";

fn plain() -> String {
    format!("{ROW}output o = s\n")
}

fn varied(patterns: &str, ports: &str) -> String {
    format!("{ROW}{patterns}v = inst.vary@1(s, {ports})\noutput o = v\n")
}

fn length(x: [f32; 3]) -> f32 {
    (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt()
}

fn column(m: &[f32; 16], c: usize) -> [f32; 3] {
    [m[c * 4], m[c * 4 + 1], m[c * 4 + 2]]
}

#[test]
fn the_operator_is_registered_and_ready() {
    let registry = builtin_registry();
    assert!(
        registry
            .ready()
            .any(|spec| spec.id.to_string() == "inst.vary@1")
    );
}

#[test]
fn no_patterns_change_nothing() {
    let base = matrices(&plain());
    let same = matrices(&varied("", ""));
    assert_eq!(base, same);
}

#[test]
fn a_scale_pattern_scales_each_placement_by_what_it_says_where_it_stands() {
    let base = matrices(&plain());
    let patterns = "w = field.expr@1(code=\"out = 1.0 + step(30.0, p.x)\")\n";
    let result = matrices(&varied(patterns, "scale=w"));
    assert_eq!(result.len(), base.len(), "the same number of placements");
    for (before, after) in base.iter().zip(&result) {
        let want = if before[12] >= 30.0 { 2.0 } else { 1.0 };
        let got = length(column(after, 0)) / length(column(before, 0));
        assert!(
            (got - want).abs() < 1e-4,
            "scale {got}, wanted {want}, at x {}",
            before[12]
        );
        assert_eq!(
            [after[12], after[13], after[14]],
            [before[12], before[13], before[14]],
            "scaling is about the placement's own origin"
        );
    }
    assert!(
        result.iter().any(|m| m[12] >= 30.0),
        "the fixture has placements on both sides of the edge"
    );
}

#[test]
fn a_yaw_pattern_turns_placements_about_up() {
    let base = matrices(&plain());
    let patterns = "w = field.expr@1(code=\"out = 1.5707964\")\n";
    let result = matrices(&varied(patterns, "yaw=w"));
    for (before, after) in base.iter().zip(&result) {
        let x = column(after, 0);
        assert!(
            (x[0]).abs() < 1e-4 && (x[2] + 1.0).abs() < 1e-4 && x[1].abs() < 1e-4,
            "a quarter turn takes +x to -z: {x:?}"
        );
        assert!(
            (length(x) - length(column(before, 0))).abs() < 1e-4,
            "turning keeps the size"
        );
        assert_eq!(
            [after[12], after[13], after[14]],
            [before[12], before[13], before[14]]
        );
    }
}

#[test]
fn a_lift_pattern_raises_placements() {
    let base = matrices(&plain());
    let patterns = "w = field.expr@1(code=\"out = 0.5 * step(30.0, p.x)\")\n";
    let result = matrices(&varied(patterns, "lift=w"));
    for (before, after) in base.iter().zip(&result) {
        let want = before[13] + if before[12] >= 30.0 { 0.5 } else { 0.0 };
        assert!(
            (after[13] - want).abs() < 1e-4,
            "y {} wanted {want}",
            after[13]
        );
        assert_eq!((after[12], after[14]), (before[12], before[14]));
    }
}

#[test]
fn noise_in_a_pattern_gives_each_placement_its_own_value() {
    let patterns = "w = field.expr@1(code=\"out = 0.5 + 0.5 * perlin(p * 0.37, 3.0)\")\n";
    let result = matrices(&varied(patterns, "scale=w"));
    let sizes: Vec<f32> = result.iter().map(|m| length(column(m, 0))).collect();
    let (low, high) = sizes
        .iter()
        .fold((f32::MAX, f32::MIN), |(a, b), s| (a.min(*s), b.max(*s)));
    assert!(high - low > 0.01, "placements differ: {sizes:?}");
    assert_eq!(
        sizes,
        matrices(&varied(patterns, "scale=w"))
            .iter()
            .map(|m| length(column(m, 0)))
            .collect::<Vec<_>>(),
        "and the same every time"
    );
}

#[test]
fn a_pattern_that_is_not_a_number_is_refused_with_the_node_named() {
    let patterns = "w = field.expr@1(code=\"out = vec3(1.0, 2.0, 3.0)\")\n";
    let error = evaluate(&varied(patterns, "scale=w")).unwrap_err();
    assert!(error.contains("pattern.not_float"), "{error}");
}

#[test]
fn only_a_set_can_be_varied() {
    let error = evaluate("b = solid.box@1(size=[1, 1, 1])\nw = field.expr@1(code=\"out = 1.0\")\nv = inst.vary@1(b, scale=w)\noutput o = v\n").unwrap_err();
    assert!(error.contains("graph.port_type"), "{error}");
}

#[test]
fn scale_is_kept_sane() {
    let patterns = "w = field.expr@1(code=\"out = 1000.0\")\n";
    let error = evaluate(&varied(patterns, "scale=w")).unwrap_err();
    assert!(
        error.contains("param.range") || error.contains("pattern.range"),
        "a scale above the limit is refused with a suggestion: {error}"
    );
}
