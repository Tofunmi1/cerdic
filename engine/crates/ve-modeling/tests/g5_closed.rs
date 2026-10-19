//! Closed sweep path acceptance cases.
use serde_json::{Value, json};
use ve_graph::{Outcome, Session};
use ve_modeling::{Evaluator, Quality};

fn evaluate(
    rows: &[(&str, &str, Value, &[&str])],
    last: &str,
) -> Result<ve_modeling::Evaluation, String> {
    let mut session = Session::with_builtin(3);
    let mut commands = Vec::new();
    for (alias, op, params, inputs) in rows {
        commands.push(json!({"add_node": {"op": op, "params": params, "alias": alias}}));
        for input in *inputs {
            let (port, source) = input.split_once(':').unwrap_or(("in", input));
            commands.push(json!({"connect": {"from": {"node": source, "port": "out"}, "to": {"node": alias, "port": port}}}));
        }
    }
    commands.push(json!({"set_output": {"name": "part", "from": {"node": last, "port": "out"}}}));
    let batch = json!({"doc": session.document().id, "base_revision": 0, "transaction": "t_g5_closed", "commands": commands});
    if let outcome @ Outcome::Rejected { .. } = session.apply_json(&batch.to_string()) {
        return Err(format!("fixture command batch rejected: {outcome:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .map_err(|error| format!("{}: {}", error.code, error.message))
}

fn torus_program(segments: u64) -> Result<ve_modeling::Evaluation, String> {
    evaluate(
        &[
            (
                "$profile",
                "profile.circle@1",
                json!({"radius": 0.1, "segments": 48}),
                &[],
            ),
            (
                "$path",
                "curve.circle@1",
                json!({"center": [2.0, 0.0, 0.0], "radius": 1.0, "segments": segments}),
                &[],
            ),
            (
                "$sweep",
                "solid.sweep@1",
                json!({}),
                &["curve:$path", "profile:$profile"],
            ),
        ],
        "$sweep",
    )
}

#[test]
fn g5_01_closed_circle_sweep_matches_pappus_volume() {
    let result = torus_program(48).unwrap();
    let expected = 2.0 * std::f64::consts::PI * std::f64::consts::PI * 0.1_f64.powi(2);
    let actual = result.parts[0].volume;
    assert!(
        (actual - expected).abs() / expected < 0.01,
        "{actual} vs {expected}"
    );
    let (minimum, maximum) = bounds(&result.parts[0].positions);
    assert!((f64::from(minimum[0]) - 0.9).abs() < 0.01);
    assert!((f64::from(maximum[0]) - 3.1).abs() < 0.01);
    assert!((f64::from(minimum[1]) + 0.1).abs() < 0.01);
    assert!((f64::from(maximum[1]) - 0.1).abs() < 0.01);
    assert!((f64::from(minimum[2]) + 1.1).abs() < 0.01);
    assert!((f64::from(maximum[2]) - 1.1).abs() < 0.01);
}

#[test]
fn g5_07_closed_circle_sweep_has_no_open_mesh_edges() {
    let result = torus_program(24).unwrap();
    assert_closed_mesh(&result);
}

#[test]
fn g5_07_closed_authored_polyline_sweep_has_no_open_mesh_edges() {
    let result = evaluate(
        &[
            (
                "$section",
                "profile.circle@1",
                json!({"radius": 0.03, "segments": 12}),
                &[],
            ),
            (
                "$path",
                "curve.polyline@1",
                json!({"closed": true, "points": [[0,0,0],[1,0,0],[1,0,1],[0,0,1],[0,0,0]]}),
                &[],
            ),
            (
                "$sweep",
                "solid.sweep@1",
                json!({}),
                &["curve:$path", "profile:$section"],
            ),
        ],
        "$sweep",
    )
    .unwrap();
    assert_closed_mesh(&result);
}

#[test]
fn g5_07_closed_cubic_bezier_sweep_has_no_open_mesh_edges() {
    let result = evaluate(
        &[
            ("$section", "profile.circle@1", json!({"radius": 0.02, "segments": 12}), &[]),
            ("$path", "curve.bezier@1", json!({"closed": true, "steps": 32, "points": [[0.5,0,0],[0.5,0,0.5],[-0.5,0,0.5],[0.5,0,0]]}), &[]),
            ("$sweep", "solid.sweep@1", json!({}), &["curve:$path", "profile:$section"]),
        ],
        "$sweep",
    )
    .unwrap();
    assert_closed_mesh(&result);
}

fn assert_closed_mesh(result: &ve_modeling::Evaluation) {
    let mut edges = std::collections::BTreeMap::new();
    for triangle in result.parts[0].indices.chunks_exact(3) {
        for (a, b) in [
            (triangle[0], triangle[1]),
            (triangle[1], triangle[2]),
            (triangle[2], triangle[0]),
        ] {
            *edges.entry((a.min(b), a.max(b))).or_insert(0_usize) += 1;
        }
    }
    assert!(edges.values().all(|count| *count == 2));
}

fn bounds(points: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    points.iter().fold(
        ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]),
        |(mut minimum, mut maximum), point| {
            for axis in 0..3 {
                minimum[axis] = minimum[axis].min(point[axis]);
                maximum[axis] = maximum[axis].max(point[axis]);
            }
            (minimum, maximum)
        },
    )
}
