//! `curve.hang` is a rope, chain or cable hanging between two points under its own weight: a catenary of a chosen length.
//! The lowest point is checked against the closed form (a = scale, length = 2a sinh(span / 2a), sag = a (cosh(span / 2a) - 1)).
//! Written before the body; do not weaken.
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
        transaction: TransactionId::parse("t_g22_hang").unwrap(),
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

/// A thin tube along the rope, so its vertices say where the rope runs.
fn rope(params: &str) -> Result<Vec<[f32; 3]>, String> {
    let source = format!(
        "c = curve.hang@1({params})\nt = curve.tube@1(c, radius=0.01, sides=4)\noutput t = t\n"
    );
    Ok(evaluate(&source)?.parts[0].positions.clone())
}

fn lowest(points: &[[f32; 3]]) -> f32 {
    points.iter().map(|p| p[1]).fold(f32::MAX, f32::min)
}

#[test]
fn a_rope_of_five_percent_slack_sags_by_the_catenary_closed_form() {
    // 10 m span, length 10.5 m: a = 9.1964, sag = 1.3930 m.
    let points = rope("from=[0, 3, 0], to=[10, 3, 0], slack=0.05").unwrap();
    assert!(
        (lowest(&points) - (3.0 - 1.3930 - 0.01)).abs() < 0.03,
        "lowest point {}",
        lowest(&points)
    );
}

#[test]
fn more_slack_sags_further_by_the_closed_form() {
    // length 12 m over a 10 m span: a = 4.6954, sag = 2.9234 m.
    let points = rope("from=[0, 3, 0], to=[10, 3, 0], slack=0.2").unwrap();
    assert!(
        (lowest(&points) - (3.0 - 2.9234 - 0.01)).abs() < 0.04,
        "lowest point {}",
        lowest(&points)
    );
}

#[test]
fn the_rope_ends_where_it_is_tied_and_has_the_asked_length() {
    let points = rope("from=[0, 3, 0], to=[10, 3, 0], slack=0.05, points=64").unwrap();
    let (lo, hi) = points
        .iter()
        .fold((f32::MAX, f32::MIN), |(l, h), p| (l.min(p[0]), h.max(p[0])));
    assert!(
        (lo + 0.0).abs() < 0.02 && (hi - 10.0).abs() < 0.02,
        "the ends are tied at x = 0 and 10: {lo}..{hi}"
    );
    let top = points.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
    assert!(
        (top - 3.0).abs() < 0.03,
        "the ends are at the tie height: {top}"
    );
}

#[test]
fn no_slack_is_a_straight_line() {
    let points = rope("from=[0, 3, 0], to=[10, 3, 0], slack=0").unwrap();
    assert!(
        (lowest(&points) - 2.99).abs() < 0.03,
        "a taut rope does not sag: {}",
        lowest(&points)
    );
}

#[test]
fn a_rope_between_two_heights_hangs_below_the_lower_end_when_slack_is_large() {
    let points = rope("from=[0, 6, 0], to=[8, 2, 3], slack=0.4").unwrap();
    assert!(
        lowest(&points) < 2.0 - 0.1,
        "it dips below the lower tie: {}",
        lowest(&points)
    );
}

#[test]
fn a_rope_hanging_straight_down_is_a_vertical_line() {
    let points = rope("from=[1, 6, 1], to=[1, 2, 1], slack=0.1").unwrap();
    assert!(
        points
            .iter()
            .all(|p| (p[0] - 1.0).abs() < 0.03 && (p[2] - 1.0).abs() < 0.03),
        "it stays on the vertical through both ends"
    );
}

#[test]
fn two_ends_at_the_same_point_are_refused_with_a_fix() {
    let error = rope("from=[1, 1, 1], to=[1, 1, 1], slack=0.1").unwrap_err();
    assert!(
        error.contains("from") && error.contains("to"),
        "the message names the parameters: {error}"
    );
}
