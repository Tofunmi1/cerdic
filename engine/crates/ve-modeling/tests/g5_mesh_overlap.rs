//! Regression cases for triangle-level sweep intersection detection.
use serde_json::json;
use ve_graph::{Outcome, Session};
use ve_modeling::{Evaluator, Quality};

fn tube(points: serde_json::Value) -> Result<(), String> {
    let mut session = Session::with_builtin(3);
    let batch = json!({
        "doc": session.document().id,
        "base_revision": 0,
        "transaction": "t_mesh_overlap",
        "commands": [
            {"add_node": {"op": "profile.circle@1", "params": {"radius": 0.1, "segments": 24}, "alias": "$profile"}},
            {"add_node": {"op": "curve.polyline@1", "params": {"points": points}, "alias": "$path"}},
            {"add_node": {"op": "solid.sweep@1", "alias": "$sweep"}},
            {"connect": {"from": {"node": "$profile", "port": "out"}, "to": {"node": "$sweep", "port": "profile"}}},
            {"connect": {"from": {"node": "$path", "port": "out"}, "to": {"node": "$sweep", "port": "curve"}}},
            {"set_output": {"name": "part", "from": {"node": "$sweep", "port": "out"}}}
        ]
    });
    if let Outcome::Rejected { .. } = session.apply_json(&batch.to_string()) {
        return Err("fixture command batch rejected".into());
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .map(|_| ())
        .map_err(|error| format!("{}: {}", error.code, error.message))
}

#[test]
fn g5_25_end_cap_into_distant_tube_is_refused_by_mesh_check() {
    let error = tube(json!([[0, 0, 0], [1, 0, 0], [1, 0.15, 0], [0.05, 0.15, 0]])).unwrap_err();
    assert!(error.contains("curve.sweep_self_intersection"), "{error}");
    assert!(error.contains("triangles"), "{error}");
}

#[test]
fn g5_25_bend_vertex_touch_is_refused() {
    let error = tube(json!([
        [0, 0, 0],
        [1, 0, 0],
        [1, 1, 0],
        [0.1, 0.1, 0],
        [0, 0.1, 0]
    ]))
    .unwrap_err();
    assert!(error.contains("curve.sweep_self_intersection"), "{error}");
    assert!(error.contains("triangles"), "{error}");
}

#[test]
fn g5_25_parallel_overlap_is_refused() {
    let error = tube(json!([
        [0, 0, 0],
        [2, 0, 0],
        [2, 1, 0],
        [0, 1, 0],
        [0, 0.15, 0],
        [2, 0.15, 0]
    ]))
    .unwrap_err();
    assert!(error.contains("curve.sweep_self_intersection"), "{error}");
    assert!(error.contains("triangles"), "{error}");
}

#[test]
fn g5_25_near_miss_still_builds() {
    let result = tube(json!([
        [0, 0, 0],
        [2, 0, 0],
        [2.5, 0.125, 0],
        [2, 0.25, 0],
        [0, 0.25, 0]
    ]));
    assert!(result.is_ok(), "{result:?}");
}

/// A valid straight or gently winding tube must build however many path points it has. The first
/// version of the triangle check refused a straight 200-point tube with `budget.triangles` because it
/// counted every pair sharing a grid cell instead of every pair with overlapping boxes.
#[test]
fn g5_25_dense_valid_tubes_are_not_refused_by_the_overlap_check() {
    let straight = (0..200)
        .map(|i| json!([0.0, f64::from(i) * 0.05, 0.0]))
        .collect::<Vec<_>>();
    let winding = (0..200)
        .map(|i| {
            let t = f64::from(i) * 0.1;
            json!([0.3 * t.sin(), f64::from(i) * 0.05, 0.3 * (t * 0.7).cos()])
        })
        .collect::<Vec<_>>();
    for (name, path) in [("straight", straight), ("winding", winding)] {
        let result = tube(json!(path));
        assert!(result.is_ok(), "{name}: {result:?}");
    }
}
