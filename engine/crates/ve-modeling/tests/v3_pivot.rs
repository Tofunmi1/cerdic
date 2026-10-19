//! V3-04: the transform chain a pivoted viewport edit authors (move by minus the pivot, transform,
//! move by the pivot, see `web/panels/transform-commands.js`) leaves the pivot fixed in the real kernel.
//! The box corner sits on the pivot, so after a rotation or a scale it must still be the bounds corner.
//! Text angles are degrees; the document stores radians (`xform.rotate` angles, ADR 0015).
#![expect(clippy::unwrap_used, reason = "test fixture construction")]
use ve_graph::{Outcome, Session, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

/// Bounds of the single part of `chain` applied to a box whose minimum corner is (1, 2, 3).
fn bounds_after(chain: &str) -> ([f32; 3], [f32; 3]) {
    let source = format!(
        "b = solid.box(size=[2,4,6])\nplaced = xform.move(b, by=[2,4,6])\n{chain}output result"
    );
    let parsed = parse_text(&builtin_registry(), &source).unwrap();
    let mut session = Session::with_builtin(5);
    let batch = serde_json::json!({"doc": session.document().id, "base_revision": 0,
        "transaction": "t_pivot", "commands": parsed.commands});
    assert!(matches!(
        session.apply_json(&batch.to_string()),
        Outcome::Committed { .. }
    ));
    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .unwrap();
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

fn close(actual: [f32; 3], expected: [f32; 3], why: &str) {
    assert!(
        actual
            .into_iter()
            .zip(expected)
            .all(|(a, e)| (a - e).abs() < 1.0e-4),
        "{why}: {actual:?} vs {expected:?}"
    );
}

#[test]
fn v3_04_rotation_about_a_corner_pivot_keeps_the_corner() {
    // A quarter turn about Z around (1, 2, 3): x' = 1 - (y - 2), y' = 2 + (x - 1).
    let (min, max) = bounds_after(
        "a = xform.move(placed, by=[-1,-2,-3])\nr = xform.rotate(a, angles=[0, 0, 90])\nresult = xform.move(r, by=[1,2,3])\n",
    );
    close(min, [-3.0, 2.0, 3.0], "min after a quarter turn");
    close(max, [1.0, 4.0, 9.0], "max after a quarter turn");
}

#[test]
fn v3_04_scale_about_a_corner_pivot_keeps_the_corner() {
    let (min, max) = bounds_after(
        "a = xform.move(placed, by=[-1,-2,-3])\ns = xform.scale(a, by=[2, 1, 1])\nresult = xform.move(s, by=[1,2,3])\n",
    );
    close(min, [1.0, 2.0, 3.0], "min after doubling along X");
    close(max, [5.0, 6.0, 9.0], "max after doubling along X");
}

#[test]
fn v3_03_a_mirrored_part_edits_in_world_space_like_any_other() {
    // `xform.mirror` keeps the original and adds its reflection, so the result spans x in [-3, 3],
    // y in [2, 6], z in [3, 9]. A viewport edit is a chain of world-space transforms after it: a quarter
    // turn about Z around (0, 4, 6) gives x' = -(y - 4), y' = 4 + x, whatever the part's handedness.
    let (min, max) = bounds_after(
        "m = xform.mirror(placed, axis=\"x\")\na = xform.move(m, by=[0,-4,-6])\nr = xform.rotate(a, angles=[0, 0, 90])\nresult = xform.move(r, by=[0,4,6])\n",
    );
    close(min, [-2.0, 1.0, 3.0], "min of the turned mirrored part");
    close(max, [2.0, 7.0, 9.0], "max of the turned mirrored part");
}

#[test]
fn v3_04_without_the_moves_the_same_rotation_swings_the_box_about_the_origin() {
    // The control: the pivot moves make the difference, so the tests above can fail.
    let (min, _) = bounds_after("result = xform.rotate(placed, angles=[0, 0, 90])\n");
    close(
        min,
        [-6.0, 1.0, 3.0],
        "min when rotating about the origin instead",
    );
}
