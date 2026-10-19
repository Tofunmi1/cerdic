//! Initial G5 curve and sweep conformance cases.
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
    let batch = json!({"doc": session.document().id, "base_revision": 0, "transaction": "t_g5_surfaces", "commands": commands});
    if let outcome @ Outcome::Rejected { .. } = session.apply_json(&batch.to_string()) {
        return Err(format!("fixture command batch rejected: {outcome:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .map_err(|error| format!("{}: {}", error.code, error.message))
}

#[test]
fn g5_15_curve_polyline_is_an_intermediate_value() {
    let result = evaluate(
        &[(
            "$curve",
            "curve.polyline@1",
            json!({"points": [[0,0,0],[0,1,0],[0,2,0]]}),
            &[],
        )],
        "$curve",
    );
    let error = result.unwrap_err();
    assert!(error.contains("graph.port_type"), "{error}");
}

#[test]
fn g5_16_linear_sweep_has_closed_analytic_volume() {
    let result = evaluate(
        &[
            (
                "$profile",
                "profile.circle@1",
                json!({"radius": 0.25, "segments": 48}),
                &[],
            ),
            (
                "$path",
                "curve.polyline@1",
                json!({"points": [[0,0,0],[0,2,0]]}),
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
    .unwrap();
    let volume = result.parts[0].volume;
    assert!(
        (volume - std::f64::consts::PI * 0.25_f64.powi(2) * 2.0).abs() < 0.0039,
        "{volume}"
    );
}

#[test]
fn g5_06_repeated_path_points_are_refused() {
    let result = evaluate(
        &[
            (
                "$profile",
                "profile.circle@1",
                json!({"radius": 0.25, "segments": 16}),
                &[],
            ),
            (
                "$path",
                "curve.polyline@1",
                json!({"points": [[0,0,0],[0,1,0],[0,1,0],[0,2,0]]}),
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
    );
    let error = result.unwrap_err();
    assert!(error.contains("geometry.degenerate_curve"), "{error}");
}

#[test]
fn g5_17_cubic_bezier_sweep_keeps_a_closed_solid() {
    let result = evaluate(
        &[
            (
                "$profile",
                "profile.circle@1",
                json!({"radius": 0.1, "segments": 24}),
                &[],
            ),
            (
                "$path",
                "curve.bezier@1",
                json!({"points": [[0,0,0],[0,1,0],[1,2,0],[1,3,0]], "steps": 20}),
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
    .unwrap();
    assert!(result.parts[0].positions.iter().any(|point| point[0] > 0.8));
}

#[test]
fn g5_20_equal_section_loft_matches_extrusion_volume_and_bounds() {
    let loft = evaluate(
        &[
            ("$a", "profile.rect@1", json!({"size": [1.0, 1.0]}), &[]),
            ("$b", "profile.rect@1", json!({"size": [1.0, 1.0]}), &[]),
            (
                "$loft",
                "solid.loft@1",
                json!({"spacing": 2.0}),
                &["sections:$a", "sections:$b"],
            ),
        ],
        "$loft",
    )
    .unwrap();
    let extruded = evaluate(
        &[
            (
                "$profile",
                "profile.rect@1",
                json!({"size": [1.0, 1.0]}),
                &[],
            ),
            (
                "$extrude",
                "solid.extrude@1",
                json!({"height": 2.0, "divisions": 1, "twist": 0.0, "top_scale": [1.0, 1.0]}),
                &["$profile"],
            ),
        ],
        "$extrude",
    )
    .unwrap();
    assert!(
        (loft.parts[0].volume - extruded.parts[0].volume).abs() < 1.0e-5,
        "loft {}, extrusion {}",
        loft.parts[0].volume,
        extruded.parts[0].volume
    );
    let loft_y = loft.parts[0]
        .positions
        .iter()
        .map(|point| point[1])
        .fold(f32::MIN, f32::max);
    let extrude_y = extruded.parts[0]
        .positions
        .iter()
        .map(|point| point[1])
        .fold(f32::MIN, f32::max);
    assert!((loft_y - extrude_y).abs() < 1.0e-5);
}

#[test]
fn g5_21_loft_accepts_varying_convex_sections_for_a_vase_body() {
    let result = evaluate(
        &[
            (
                "$bottom",
                "profile.circle@1",
                json!({"radius": 0.2, "segments": 48}),
                &[],
            ),
            (
                "$waist",
                "profile.circle@1",
                json!({"radius": 0.1, "segments": 48}),
                &[],
            ),
            (
                "$rim",
                "profile.circle@1",
                json!({"radius": 0.15, "segments": 48}),
                &[],
            ),
            (
                "$body",
                "solid.loft@1",
                json!({"spacing": 0.2}),
                &["sections:$bottom", "sections:$waist", "sections:$rim"],
            ),
        ],
        "$body",
    )
    .unwrap();
    let max_y = result.parts[0]
        .positions
        .iter()
        .map(|point| point[1])
        .fold(f32::MIN, f32::max);
    assert!((f64::from(max_y) - 0.4).abs() < 1.0e-6);
    assert!((result.parts[0].volume - 0.024_609).abs() < 0.0003);
}

#[test]
fn g5_18_oversized_sweep_is_refused_during_preflight() {
    let error = evaluate(
        &[
            (
                "$profile",
                "profile.circle@1",
                json!({"radius": 0.1, "segments": 512}),
                &[],
            ),
            (
                "$path",
                "curve.bezier@1",
                json!({"points": [[0,0,0],[0,1,0],[1,2,0],[1,3,0]], "steps": 512}),
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
    .unwrap_err();
    assert!(error.contains("budget.triangles"), "{error}");
}

#[test]
fn g5_05_tube_larger_than_local_curvature_radius_is_refused() {
    let error = evaluate(
        &[
            (
                "$profile",
                "profile.circle@1",
                json!({"radius": 0.6, "segments": 24}),
                &[],
            ),
            (
                "$path",
                "curve.polyline@1",
                json!({"points": [[0,0,0],[1,0,0],[1,0.1,0]]}),
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
    .unwrap_err();
    assert!(error.contains("curve.sweep_curvature"), "{error}");
    assert!(error.contains("tube radius"), "{error}");
    assert!(error.contains("curvature radius"), "{error}");
}

/// A round tube of radius 0.1 swept along `points`.
fn tube_along(points: &Value) -> Result<ve_modeling::Evaluation, String> {
    evaluate(
        &[
            (
                "$profile",
                "profile.circle@1",
                json!({"radius": 0.1, "segments": 24}),
                &[],
            ),
            (
                "$path",
                "curve.polyline@1",
                json!({ "points": points }),
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
fn g5_25_a_path_that_crosses_itself_is_refused_with_the_segments_named() {
    // The last segment runs down through the middle of the first one.
    let error = tube_along(&json!([
        [0, 0, 0],
        [4, 0, 0],
        [4, 2, 0],
        [2, 2, 0],
        [2, -2, 0]
    ]))
    .unwrap_err();
    assert!(error.contains("curve.sweep_self_intersection"), "{error}");
    assert!(error.contains("segments 0 and 3"), "{error}");
}

#[test]
fn g5_25_tubes_closer_than_twice_the_inscribed_radius_overlap_and_farther_ones_do_not() {
    // Two passes 0.15 apart (tube radius 0.1: the tubes overlap) and 0.25 apart (a gap of 0.05).
    let overlapping = tube_along(&json!([
        [0, 0, 0],
        [4, 0, 0],
        [4, 2, 0],
        [2, 2, 0.15],
        [2, -2, 0.15]
    ]))
    .unwrap_err();
    assert!(
        overlapping.contains("curve.sweep_self_intersection"),
        "{overlapping}"
    );
    let clear = tube_along(&json!([
        [0, 0, 0],
        [4, 0, 0],
        [4, 2, 0],
        [2, 2, 0.25],
        [2, -2, 0.25]
    ]));
    assert!(clear.is_ok(), "{clear:?}");
}

#[test]
fn g5_25_a_path_that_stops_short_of_the_crossing_is_accepted() {
    let result = tube_along(&json!([
        [0, 0, 0],
        [4, 0, 0],
        [4, 2, 0],
        [2, 2, 0],
        [2, 0.5, 0]
    ]));
    assert!(result.is_ok(), "{result:?}");
}
