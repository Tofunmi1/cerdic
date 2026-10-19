//! G5 unsupported section topology cases.
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
    let batch = json!({"doc": session.document().id, "base_revision": 0, "transaction": "t_g5_refusals", "commands": commands});
    if let outcome @ Outcome::Rejected { .. } = session.apply_json(&batch.to_string()) {
        return Err(format!("fixture command batch rejected: {outcome:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .map_err(|error| format!("{}: {}", error.code, error.message))
}

#[test]
fn g5_08_loft_refuses_sections_with_different_point_counts() {
    let error = evaluate(
        &[
            (
                "$a",
                "profile.circle@1",
                json!({"radius": 0.2, "segments": 16}),
                &[],
            ),
            (
                "$b",
                "profile.circle@1",
                json!({"radius": 0.1, "segments": 24}),
                &[],
            ),
            (
                "$loft",
                "solid.loft@1",
                json!({"spacing": 0.2}),
                &["sections:$a", "sections:$b"],
            ),
        ],
        "$loft",
    )
    .unwrap_err();
    assert!(error.contains("surface.incompatible_sections"), "{error}");
}

#[test]
fn g5_08_loft_refuses_a_profile_with_a_hole() {
    let error = evaluate(
        &[
            (
                "$outer",
                "profile.circle@1",
                json!({"radius": 0.3, "segments": 24}),
                &[],
            ),
            (
                "$inner",
                "profile.circle@1",
                json!({"radius": 0.1, "segments": 24}),
                &[],
            ),
            (
                "$ring",
                "profile.difference@1",
                json!({}),
                &["in:$outer", "in:$inner"],
            ),
            (
                "$other",
                "profile.circle@1",
                json!({"radius": 0.25, "segments": 24}),
                &[],
            ),
            (
                "$loft",
                "solid.loft@1",
                json!({"spacing": 0.2}),
                &["sections:$ring", "sections:$other"],
            ),
        ],
        "$loft",
    )
    .unwrap_err();
    assert!(error.contains("surface.incompatible_sections"), "{error}");
}

#[test]
fn g5_08_loft_refuses_an_empty_zero_area_section() {
    let error = evaluate(
        &[
            (
                "$outer",
                "profile.circle@1",
                json!({"radius": 0.3, "segments": 24}),
                &[],
            ),
            (
                "$same",
                "profile.circle@1",
                json!({"radius": 0.3, "segments": 24}),
                &[],
            ),
            (
                "$empty",
                "profile.difference@1",
                json!({}),
                &["in:$outer", "in:$same"],
            ),
            (
                "$valid",
                "profile.circle@1",
                json!({"radius": 0.2, "segments": 24}),
                &[],
            ),
            (
                "$loft",
                "solid.loft@1",
                json!({"spacing": 0.2}),
                &["sections:$empty", "sections:$valid"],
            ),
        ],
        "$loft",
    )
    .unwrap_err();
    assert!(error.contains("surface.degenerate_section"), "{error}");
}

#[test]
fn g5_07_closed_polyline_refuses_a_degenerate_seam() {
    let error = evaluate(
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
                json!({"closed": true, "points": [[0,0,0],[1,0,0],[0,0,0]]}),
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
    .unwrap_err();
    assert!(error.contains("geometry.invalid_curve"), "{error}");
}

#[test]
fn g5_07_closed_bezier_requires_matching_endpoints() {
    let error = evaluate(
        &[
            ("$section", "profile.circle@1", json!({"radius": 0.03, "segments": 12}), &[]),
            ("$path", "curve.bezier@1", json!({"closed": true, "points": [[0,0,0],[0,0,0.5],[0.5,0,0.5],[1,0,0]], "steps": 16}), &[]),
            ("$sweep", "solid.sweep@1", json!({}), &["curve:$path", "profile:$section"]),
        ],
        "$sweep",
    )
    .unwrap_err();
    assert!(error.contains("geometry.invalid_curve"), "{error}");
}
