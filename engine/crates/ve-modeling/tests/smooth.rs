//! `profile.smooth`: corner cutting turns a few points into a smooth outline.
#![expect(
    clippy::unwrap_used,
    reason = "case helpers unwrap fixtures that are known to be valid"
)]
use serde_json::json;
use ve_graph::{Outcome, Session, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn volume(program: &str) -> f64 {
    let mut session = Session::with_builtin(8);
    let batch = parse_text(&builtin_registry(), program).unwrap();
    let batch = json!({"doc": session.document().id, "base_revision": 0, "transaction": "t_smooth", "commands": batch.commands});
    assert!(matches!(
        session.apply_json(&batch.to_string()),
        Outcome::Committed { .. }
    ));
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap()
        .parts[0]
        .volume
}

const SQUARE: &str = "p = profile.polygon@1(points=[[0, 0], [1, 0], [1, 1], [0, 1]])\n";

#[test]
fn corner_cutting_rounds_a_square_and_shrinks_it_slightly() {
    let sharp = volume(&format!(
        "{SQUARE}e = solid.extrude@1(p, height=1)\noutput e\n"
    ));
    let once = volume(&format!(
        "{SQUARE}s = profile.smooth@1(p, iterations=1)\ne = solid.extrude@1(s, height=1)\noutput e\n"
    ));
    let thrice = volume(&format!(
        "{SQUARE}s = profile.smooth@1(p, iterations=3)\ne = solid.extrude@1(s, height=1)\noutput e\n"
    ));
    // One round cuts four corner triangles of 1/32 each: an octagon of area 0.875.
    assert!((once - 0.875).abs() < 1e-9, "{once}");
    assert!(thrice < once && once < sharp, "{thrice} {once} {sharp}");
    assert!(
        thrice > 0.75,
        "it converges on a curve, it does not collapse: {thrice}"
    );
}

#[test]
fn a_smoothed_vase_profile_still_revolves_and_keeps_its_axis_edge() {
    let outer = "outer = profile.polygon@1(points=[[0, 0], [0.06, 0], [0.11, 0.12], [0.07, 0.26], [0.05, 0.3], [0, 0.3]])\n";
    let raw = volume(&format!(
        "{outer}b = solid.revolve@1(outer, segments=48)\noutput b\n"
    ));
    let soft = volume(&format!(
        "{outer}s = profile.smooth@1(outer, iterations=3)\nb = solid.revolve@1(s, segments=48)\noutput b\n"
    ));
    assert!(
        soft < raw && soft > 0.8 * raw,
        "smoothed {soft} against raw {raw}"
    );
}

#[test]
fn iterations_are_bounded() {
    // One to five rounds only: six is refused when the text is read, with its line and column.
    let program = format!("{SQUARE}s = profile.smooth@1(p, iterations=6)\n");
    let errors = parse_text(&builtin_registry(), &program).unwrap_err();
    assert_eq!(errors[0].code, "param.range");
    assert_eq!(errors[0].limit, Some(5.0));
}
