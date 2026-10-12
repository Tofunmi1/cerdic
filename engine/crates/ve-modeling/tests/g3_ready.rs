//! Conformance cases that let the primitive, boolean and transform operators leave draft.
#![expect(
    clippy::unwrap_used,
    reason = "case helpers unwrap fixtures that are known to be valid"
)]
use serde_json::{Value, json};
use std::f64::consts::PI;
use ve_graph::{Outcome, Session};
use ve_modeling::{Evaluation, Evaluator, Quality};

/// Builds a chain from `(alias, op, params, inputs)` rows; the last row is the output.
fn build(rows: &[(&str, &str, Value, &[&str])]) -> Evaluation {
    let mut session = Session::with_builtin(3);
    let mut commands = Vec::new();
    for (alias, op, params, inputs) in rows {
        commands.push(json!({"add_node": {"op": op, "params": params, "alias": alias}}));
        for input in *inputs {
            commands.push(json!({"connect": {"from": {"node": input, "port": "out"}, "to": {"node": alias, "port": "in"}}}));
        }
    }
    let last = rows.last().unwrap().0;
    commands.push(json!({"set_output": {"name": "part", "from": {"node": last, "port": "out"}}}));
    let batch = json!({"doc": session.document().id, "base_revision": 0, "transaction": "t_ready", "commands": commands});
    assert!(matches!(
        session.apply_json(&batch.to_string()),
        Outcome::Committed { .. }
    ));
    Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .unwrap()
}

fn extent(result: &Evaluation, axis: usize) -> f64 {
    let values = result.parts[0].positions.iter().map(|p| f64::from(p[axis]));
    let hi = values.clone().fold(f64::MIN, f64::max);
    let lo = values.fold(f64::MAX, f64::min);
    hi - lo
}

fn volume(rows: &[(&str, &str, Value, &[&str])]) -> f64 {
    build(rows).parts[0].volume
}

fn near(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance * b.abs().max(1e-12)
}

const BOX: (&str, &str) = ("$a", "solid.box@1");

#[test]
fn box_has_the_requested_size_and_volume() {
    let result = build(&[(BOX.0, BOX.1, json!({"size": [1.0, 2.0, 3.0]}), &[])]);
    assert!(near(result.parts[0].volume, 6.0, 1e-9));
    assert!(near(extent(&result, 0), 1.0, 1e-6) && near(extent(&result, 2), 3.0, 1e-6));
}

#[test]
fn sphere_and_cylinder_match_their_formulas_at_high_facet_counts() {
    let sphere = volume(&[(
        "$a",
        "solid.sphere@1",
        json!({"radius": 1.0, "segments": 128}),
        &[],
    )]);
    assert!(near(sphere, 4.0 / 3.0 * PI, 5e-3), "{sphere}");
    let cylinder = volume(&[(
        "$a",
        "solid.cylinder@1",
        json!({"radius": 0.5, "height": 2.0, "segments": 256}),
        &[],
    )]);
    assert!(near(cylinder, PI * 0.25 * 2.0, 2e-3), "{cylinder}");
}

fn two_boxes(op: &str, offset: f64) -> f64 {
    volume(&[
        ("$a", "solid.box@1", json!({"size": [1.0, 1.0, 1.0]}), &[]),
        ("$b", "solid.box@1", json!({"size": [1.0, 1.0, 1.0]}), &[]),
        (
            "$m",
            "xform.move@1",
            json!({"by": [offset, 0.0, 0.0]}),
            &["$b"],
        ),
        ("$r", op, json!({}), &["$a", "$m"]),
    ])
}

#[test]
fn booleans_give_the_analytic_volume_of_overlapping_boxes() {
    assert!(near(two_boxes("solid.union@1", 0.5), 1.5, 1e-9));
    assert!(near(two_boxes("solid.intersect@1", 0.5), 0.5, 1e-9));
    assert!(near(two_boxes("solid.difference@1", 0.5), 0.5, 1e-9));
    assert!(
        near(two_boxes("solid.union@1", 3.0), 2.0, 1e-9),
        "disjoint boxes add"
    );
}

#[test]
fn transforms_move_rotate_and_scale_as_documented() {
    let base = (
        "$a",
        "solid.box@1",
        json!({"size": [1.0, 2.0, 3.0]}),
        &[][..],
    );
    let moved = build(&[
        base.clone(),
        (
            "$m",
            "xform.move@1",
            json!({"by": [5.0, 0.0, 0.0]}),
            &["$a"],
        ),
    ]);
    let min_x = moved.parts[0]
        .positions
        .iter()
        .map(|p| f64::from(p[0]))
        .fold(f64::MAX, f64::min);
    assert!(near(min_x, 4.5, 1e-6));
    let turned = build(&[
        base.clone(),
        (
            "$r",
            "xform.rotate@1",
            json!({"angles": [PI / 2.0, 0.0, 0.0]}),
            &["$a"],
        ),
    ]);
    assert!(
        near(extent(&turned, 1), 3.0, 1e-5) && near(extent(&turned, 2), 2.0, 1e-5),
        "a quarter turn about X swaps Y and Z"
    );
    let scaled = build(&[
        base,
        (
            "$s",
            "xform.scale@1",
            json!({"by": [2.0, 1.0, 0.5]}),
            &["$a"],
        ),
    ]);
    assert!(near(scaled.parts[0].volume, 6.0, 1e-9));
    assert!(near(extent(&scaled, 0), 2.0, 1e-6) && near(extent(&scaled, 2), 1.5, 1e-6));
}
