//! Bounded mesh deformer acceptance cases.
use serde_json::{Value, json};
use ve_graph::{Outcome, Session};
use ve_modeling::{Evaluator, Quality};

fn evaluate(op: &str, params: Value) -> Result<ve_modeling::Evaluation, String> {
    evaluate_size(op, params, [0.2, 1.0, 0.2])
}

fn evaluate_size(
    op: &str,
    params: Value,
    size: [f64; 3],
) -> Result<ve_modeling::Evaluation, String> {
    let mut session = Session::with_builtin(3);
    let batch = json!({
        "doc": session.document().id,
        "base_revision": 0,
        "transaction": "t_g5_deformer",
        "commands": [
            {"add_node": {"op": "solid.box@1", "params": {"size": size}, "alias": "$box"}},
            {"add_node": {"op": op, "params": params, "alias": "$deformed"}},
            {"connect": {"from": {"node": "$box", "port": "out"}, "to": {"node": "$deformed", "port": "in"}}},
            {"set_output": {"name": "part", "from": {"node": "$deformed", "port": "out"}}}
        ]
    });
    if let outcome @ Outcome::Rejected { .. } = session.apply_json(&batch.to_string()) {
        return Err(format!("fixture command batch rejected: {outcome:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .map_err(|error| format!("{}: {}", error.code, error.message))
}

#[test]
fn g5_09_twist_changes_vertices_and_preserves_volume() {
    let result = evaluate("xform.twist@1", json!({"angle": 0.7})).unwrap();
    assert!(
        result.parts[0]
            .positions
            .iter()
            .any(|point| point[0].abs() > 0.11)
    );
    assert!(
        (result.parts[0].volume - 0.04).abs() < 1.2e-3,
        "volume {}",
        result.parts[0].volume
    );
}

#[test]
fn g5_09_taper_changes_cross_section_and_has_bounded_volume() {
    let result = evaluate("xform.taper@1", json!({"top_scale": 0.5})).unwrap();
    assert!((result.parts[0].volume - 0.04 * 7.0 / 12.0).abs() < 1.0e-4);
}

#[test]
fn g5_09_bend_changes_the_silhouette_and_keeps_a_valid_solid() {
    let result = evaluate("xform.bend@1", json!({"angle": 0.4})).unwrap();
    let max_x = result.parts[0]
        .positions
        .iter()
        .map(|point| point[0])
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(max_x > 0.15, "bend max x {max_x}");
    assert!(result.parts[0].volume.is_finite() && result.parts[0].volume > 0.0);
}

#[test]
fn g5_09_a_thin_tall_box_survives_a_hard_bend_with_local_refinement() {
    let result = evaluate_size(
        "xform.bend@1",
        json!({"angle": 1.2, "tolerance": 0.02}),
        [0.03, 2.0, 0.03],
    )
    .unwrap();
    assert_eq!(result.parts.len(), 1);
    assert!(result.parts[0].volume.is_finite() && result.parts[0].volume > 0.0);
    assert!(result.parts[0].indices.len() < 12 * 16 * 3);
}

#[test]
fn g5_09_a_twisted_loft_remains_one_valid_solid() {
    let mut session = Session::with_builtin(3);
    let batch = json!({
        "doc": session.document().id,
        "base_revision": 0,
        "transaction": "t_g5_twisted_loft",
        "commands": [
            {"add_node": {"op": "profile.circle@1", "params": {"radius": 0.25, "segments": 24}, "alias": "$p0"}},
            {"add_node": {"op": "profile.circle@1", "params": {"radius": 0.3, "segments": 24}, "alias": "$p1"}},
            {"add_node": {"op": "profile.circle@1", "params": {"radius": 0.2, "segments": 24}, "alias": "$p2"}},
            {"add_node": {"op": "solid.loft@1", "params": {"spacing": 0.4}, "alias": "$loft"}},
            {"add_node": {"op": "xform.twist@1", "params": {"angle": 1.1, "tolerance": 0.01}, "alias": "$twist"}},
            {"connect": {"from": {"node": "$p0", "port": "out"}, "to": {"node": "$loft", "port": "sections"}}},
            {"connect": {"from": {"node": "$p1", "port": "out"}, "to": {"node": "$loft", "port": "sections"}}},
            {"connect": {"from": {"node": "$p2", "port": "out"}, "to": {"node": "$loft", "port": "sections"}}},
            {"connect": {"from": {"node": "$loft", "port": "out"}, "to": {"node": "$twist", "port": "in"}}},
            {"set_output": {"name": "part", "from": {"node": "$twist", "port": "out"}}}
        ]
    });
    assert!(matches!(
        session.apply_json(&batch.to_string()),
        Outcome::Committed { .. }
    ));
    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .unwrap();
    assert_eq!(result.parts.len(), 1);
    assert!(result.parts[0].volume.is_finite() && result.parts[0].volume > 0.0);
}

/// The fold check once used absolute thresholds, so a small or thin solid (a blade a few millimetres
/// thick, a plate under a millimetre) was refused as "folded" however gentle the deformation.
#[test]
fn g5_09_gentle_deformations_of_small_thin_solids_are_not_refused() {
    let blade = evaluate_size(
        "xform.bend@1",
        json!({"angle": 0.07, "tolerance": 0.0005}),
        [0.02, 0.06, 0.003],
    );
    assert!(blade.is_ok(), "thin blade bend: {blade:?}");
    let plate = evaluate_size(
        "xform.twist@1",
        json!({"angle": 0.1, "tolerance": 0.0005}),
        [0.02, 0.0004, 0.02],
    );
    assert!(plate.is_ok(), "sub-millimetre plate twist: {plate:?}");
}

/// Boolean output can contain a zero-area face. A deformer must not refuse a valid solid because of
/// it: a grooved blade bent slightly and a hollow lofted shade twisted slightly both build.
#[test]
fn g5_09_deforming_boolean_output_with_degenerate_faces_still_builds() {
    let programs = [
        (
            "grooved blade",
            "s0 = profile.rect@1(size=[0.044, 0.009])\n\
             s1 = profile.rect@1(size=[0.04, 0.0085])\n\
             s2 = profile.rect@1(size=[0.034, 0.0075])\n\
             s3 = profile.rect@1(size=[0.022, 0.006])\n\
             s4 = profile.rect@1(size=[0.005, 0.0025])\n\
             raw = solid.loft@1(s0, s1, s2, s3, s4, spacing=0.15)\n\
             fuller = solid.box@1(size=[0.008, 0.38, 0.003])\n\
             front = xform.move@1(fuller, by=[0, 0.22, 0.0046])\n\
             back = xform.move@1(fuller, by=[0, 0.22, -0.0046])\n\
             grooves = solid.union@1(front, back)\n\
             cut = solid.difference@1(raw, grooves)\n\
             blade = xform.bend@1(cut, angle=0.07rad, tolerance=0.002)\n\
             output blade",
        ),
        (
            "hollow shade",
            "a = profile.circle@1(radius=0.12, segments=48)\n\
             b = profile.circle@1(radius=0.135, segments=48)\n\
             c = profile.circle@1(radius=0.11, segments=48)\n\
             d = profile.circle@1(radius=0.075, segments=48)\n\
             outer = solid.loft@1(a, b, c, d, spacing=0.06)\n\
             ia = profile.circle@1(radius=0.116, segments=48)\n\
             ib = profile.circle@1(radius=0.131, segments=48)\n\
             ic = profile.circle@1(radius=0.106, segments=48)\n\
             id = profile.circle@1(radius=0.071, segments=48)\n\
             inner = solid.loft@1(ia, ib, ic, id, spacing=0.0675)\n\
             low = xform.move@1(inner, by=[0, -0.0075, 0])\n\
             shell = solid.difference@1(outer, low)\n\
             shade = xform.twist@1(shell, angle=0.45rad, tolerance=0.004)\n\
             output shade",
        ),
    ];
    for (name, text) in programs {
        let batch = ve_graph::parse_text(&ve_graph::builtin_registry(), text)
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        let mut session = Session::with_builtin(2);
        let outcome = session.apply(&ve_graph::CommandBatch {
            doc: session.document().id.clone(),
            base_revision: 0,
            transaction: ve_graph::TransactionId::parse("t_degenerate")
                .unwrap_or_else(|error| panic!("{error:?}")),
            label: None,
            commands: batch.commands,
        });
        assert!(
            matches!(outcome, Outcome::Committed { .. }),
            "{name}: {outcome:?}"
        );
        let result = Evaluator::default().evaluate(session.document(), Quality::Final);
        assert!(result.is_ok(), "{name}: {:?}", result.err());
    }
}

#[test]
fn g5_09_bend_refuses_a_radius_smaller_than_half_width() {
    let error = evaluate_size("xform.bend@1", json!({"angle": 1.4}), [2.0, 1.0, 0.2]).unwrap_err();
    assert!(error.contains("curve.bend_curvature"), "{error}");
    assert!(error.contains("bend radius"), "{error}");
}

#[test]
fn g5_09_taper_deforms_a_swept_bezier_pipe() {
    let mut session = Session::with_builtin(3);
    let batch = json!({
        "doc": session.document().id,
        "base_revision": 0,
        "transaction": "t_g5_curved_deform",
        "commands": [
            {"add_node": {"op": "profile.circle@1", "params": {"radius": 0.08, "segments": 16}, "alias": "$profile"}},
            {"add_node": {"op": "curve.bezier@1", "params": {"points": [[0,0,0],[0,0.4,0],[0.3,0.6,0],[0.4,1,0]], "steps": 16}, "alias": "$path"}},
            {"add_node": {"op": "solid.sweep@1", "alias": "$sweep"}},
            {"add_node": {"op": "xform.taper@1", "params": {"top_scale": 0.8}, "alias": "$taper"}},
            {"connect": {"from": {"node": "$profile", "port": "out"}, "to": {"node": "$sweep", "port": "profile"}}},
            {"connect": {"from": {"node": "$path", "port": "out"}, "to": {"node": "$sweep", "port": "curve"}}},
            {"connect": {"from": {"node": "$sweep", "port": "out"}, "to": {"node": "$taper", "port": "in"}}},
            {"set_output": {"name": "pipe", "from": {"node": "$taper", "port": "out"}}}
        ]
    });
    assert!(matches!(
        session.apply_json(&batch.to_string()),
        Outcome::Committed { .. }
    ));
    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .unwrap();
    assert_eq!(result.parts.len(), 1);
    assert!(result.parts[0].volume.is_finite() && result.parts[0].volume > 0.0);
}

#[test]
fn g5_09_tighter_chord_tolerance_refines_more() {
    let coarse = evaluate("xform.twist@1", json!({"angle": 0.7, "tolerance": 0.1})).unwrap();
    let fine = evaluate("xform.twist@1", json!({"angle": 0.7, "tolerance": 0.01})).unwrap();
    assert!(fine.parts[0].indices.len() > coarse.parts[0].indices.len());
    let refused =
        evaluate("xform.twist@1", json!({"angle": 0.7, "tolerance": 0.0005})).unwrap_err();
    assert!(
        refused.contains("geometry.deformation_tolerance"),
        "{refused}"
    );
}
