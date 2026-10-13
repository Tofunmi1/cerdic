//! Frame transforms, reflected handedness, patterns and metadata preflight.
#![expect(clippy::unwrap_used, reason = "test fixture construction")]
use glam::{DMat4, DQuat, DVec3};
use ve_graph::{Outcome, Session, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn doc(source: &str) -> ve_graph::Document {
    let parsed = parse_text(&builtin_registry(), source).unwrap();
    let mut session = Session::with_builtin(2);
    let batch = serde_json::json!({"doc":session.document().id,"base_revision":0,"transaction":"t_fixture","commands":parsed.commands});
    assert!(matches!(
        session.apply_json(&batch.to_string()),
        Outcome::Committed { .. }
    ));
    session.document().clone()
}

fn matrix(frame: &ve_modeling::Frame) -> DMat4 {
    DMat4::from_rotation_translation(
        DQuat::from_array(frame.rotation),
        DVec3::from_array(frame.position),
    )
}

const BASE: &str = "b = solid.box(size=[0.2,0.2,0.2])\nf = frame.add(b, name=\"mount\", position=[1,2,3], rotation=[20,30,40])\n";

#[test]
fn mirrored_sockets_are_conjugated_and_right_handed_on_all_axes() {
    for (axis, scale) in [
        ("x", [-1.0, 1.0, 1.0]),
        ("y", [1.0, -1.0, 1.0]),
        ("z", [1.0, 1.0, -1.0]),
    ] {
        let doc = doc(&format!(
            "{BASE}m = xform.mirror(f, axis=\"{axis}\")\noutput m"
        ));
        let part = Evaluator::default()
            .evaluate(&doc, Quality::Preview)
            .unwrap()
            .parts
            .remove(0);
        let reflection = DMat4::from_scale(DVec3::from_array(scale));
        let expected = reflection * matrix(&part.frames["mount"]) * reflection;
        let actual = matrix(&part.frames["mirror.mount"]);
        assert!(
            expected
                .to_cols_array()
                .into_iter()
                .zip(actual.to_cols_array())
                .all(|(a, b)| (a - b).abs() < 1e-8)
        );
        assert!((actual.determinant() - 1.0).abs() < 1e-8);
        assert!(part.volume > 0.0);
    }
}

#[test]
fn move_rotate_and_nonuniform_scale_keep_socket_rigid() {
    let doc = doc(&format!(
        "{BASE}s = xform.scale(f, by=[2,3,4])\nr = xform.rotate(s, angles=[0,0,90])\nm = xform.move(r, by=[1,2,3])\noutput m"
    ));
    let result = Evaluator::default()
        .evaluate(&doc, Quality::Preview)
        .unwrap();
    let frame = &result.parts[0].frames["mount"];
    assert!((DVec3::from_array(frame.position) - DVec3::new(-5.0, 4.0, 15.0)).length() < 1e-8);
    let rigid = matrix(frame);
    assert!((rigid.determinant() - 1.0).abs() < 1e-8);
    for axis in [DVec3::X, DVec3::Y, DVec3::Z] {
        assert!((rigid.transform_vector3(axis).length() - 1.0).abs() < 1e-8);
    }
}

#[test]
fn pattern_sockets_are_namespaced_and_radial_first_copy_moves_out() {
    let doc = doc(&format!(
        "{BASE}r = xform.radial(f, count=3, radius=2)\noutput r"
    ));
    let result = Evaluator::default()
        .evaluate(&doc, Quality::Preview)
        .unwrap();
    let frames = &result.parts[0].frames;
    assert_eq!(frames.len(), 3);
    assert!(frames.contains_key("copy1.mount") && frames.contains_key("copy2.mount"));
    assert!(
        (DVec3::from_array(frames["mount"].position) - DVec3::new(3.0, 2.0, 3.0)).length() < 1e-8
    );
}

#[test]
fn socket_growth_is_refused_before_any_geometry_work() {
    let doc = doc(&format!(
        "{BASE}r = xform.repeat(f, count=129, step=[1,0,0])\noutput r"
    ));
    let mut evaluator = Evaluator::default();
    let error = evaluator.begin(&doc, Quality::Preview).unwrap_err();
    assert_eq!(error.code, "budget.sockets");
    assert_eq!(error.limit, Some(128.0));
    assert_eq!(evaluator.evaluated_nodes(), 0);
}

#[test]
fn instance_prototype_sockets_survive_mirror_and_realization() {
    let mirrored = doc(&format!(
        "{BASE}r = inst.repeat(f, count=2, step=[2,0,0])\nm = inst.mirror(r, axis=\"x\")\noutput m"
    ));
    let result = Evaluator::default()
        .evaluate(&mirrored, Quality::Preview)
        .unwrap();
    assert_eq!(result.parts.len(), 2);
    assert!(matrix(&result.parts[0].frames["mount"]).determinant() > 0.99999);
    assert!(matrix(&result.parts[1].frames["mirror.mount"]).determinant() > 0.99999);
    let doc = doc(&format!(
        "{BASE}r = inst.repeat(f, count=2, step=[2,0,0])\ns = inst.realize(r)\noutput s"
    ));
    let result = Evaluator::default()
        .evaluate(&doc, Quality::Preview)
        .unwrap();
    assert_eq!(result.parts[0].frames.len(), 2);
    assert!(
        (DVec3::from_array(result.parts[0].frames["copy1.mount"].position)
            - DVec3::new(3.0, 2.0, 3.0))
        .length()
            < 1e-8
    );
}

#[test]
fn attaching_a_profile_is_refused_with_socket_context() {
    let parsed=parse_text(&builtin_registry(),"p = profile.rect(size=[1,1])\nb = solid.box(size=[1,1,1])\na = xform.attach(p, to=b, socket=\"base\", to_socket=\"mount\")\noutput a").unwrap();
    let mut session = Session::with_builtin(2);
    let batch = serde_json::json!({"doc":session.document().id,"base_revision":0,"transaction":"t_bad","commands":parsed.commands});
    let diagnostics = match session.apply_json(&batch.to_string()) {
        Outcome::Rejected { diagnostics } => diagnostics,
        _ => Vec::new(),
    };
    assert!(diagnostics.iter().any(|d| d.code == "graph.port_type"));
}
