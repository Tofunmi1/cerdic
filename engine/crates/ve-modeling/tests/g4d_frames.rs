//! Named sockets, group propagation and dependent attachment acceptance.
#![expect(clippy::unwrap_used, reason = "test fixture construction")]
use glam::{DMat4, DQuat, DVec3};
use ve_graph::{Outcome, Session, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn session(source: &str) -> Session {
    let parsed = parse_text(&builtin_registry(), source).unwrap();
    let mut session = Session::with_builtin(4);
    let batch = serde_json::json!({"doc":session.document().id,"base_revision":0,
        "transaction":"t_fixture","commands":parsed.commands});
    assert!(matches!(
        session.apply_json(&batch.to_string()),
        Outcome::Committed { .. }
    ));
    session
}

fn matrix(frame: &ve_modeling::Frame) -> DMat4 {
    DMat4::from_rotation_translation(
        DQuat::from_array(frame.rotation),
        DVec3::from_array(frame.position),
    )
}

fn close(a: DMat4, b: DMat4) {
    assert!(
        a.to_cols_array()
            .into_iter()
            .zip(b.to_cols_array())
            .all(|(x, y)| (x - y).abs() < 1e-8)
    );
}

#[test]
fn g4d_01_group_result_carries_named_oriented_socket() {
    let session = session(
        "group wheel\n b = solid.box(size=[1,1,1])\n f = frame.add(b, name=\"hub\", position=[0,0.5,0], rotation=[0,0,90])\n result f\nend\nw = wheel()\noutput w",
    );
    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .unwrap();
    let frame = &result.parts[0].frames["hub"];
    assert!((matrix(frame).transform_vector3(DVec3::X) - DVec3::Y).length() < 1e-8);
    assert!((DVec3::from_array(frame.position) - DVec3::new(0.0, 0.5, 0.0)).length() < 1e-8);
    assert_eq!(session.document().groups[0].version, 1);
}

const ASSEMBLY: &str = "b = solid.box(size=[2,1,1])\np = frame.add(b, name=\"mount\", position=[0,0.5,0], rotation=[0,0,90])\nparent = xform.move(p, by=[3,1,0])\nc = solid.box(size=[0.5,1,0.5])\ns = frame.add(c, name=\"base\", position=[0,-0.5,0])\nchild = xform.attach(s, to=parent, socket=\"base\", to_socket=\"mount\")\nu = solid.box(size=[0.1,0.1,0.1])\noutput parent\noutput child\noutput unrelated = u";

#[test]
fn g4d_02_attachment_coincides_in_position_and_orientation() {
    let session = session(ASSEMBLY);
    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .unwrap();
    close(
        matrix(&result.parts[0].frames["mount"]),
        matrix(&result.parts[1].frames["base"]),
    );
    assert!((result.parts[1].volume - 0.25).abs() < 1e-8);
    assert!(matrix(&result.parts[1].frames["base"]).determinant() > 0.99999);
}

#[test]
fn g4d_03_parent_edit_moves_attached_child_and_reuses_unrelated_cache() {
    let mut session = session(ASSEMBLY);
    let mut evaluator = Evaluator::default();
    let before = evaluator
        .evaluate(session.document(), Quality::Preview)
        .unwrap();
    let parent = session
        .document()
        .nodes
        .iter()
        .find(|n| n.label.as_deref() == Some("parent"))
        .unwrap()
        .id
        .clone();
    let batch = serde_json::json!({"doc":session.document().id,"base_revision":session.document().revision,
        "transaction":"t_parent","commands":[{"set_param":{"node":parent,"param":"by","value":[5,1,0]}}]});
    assert!(matches!(
        session.apply_json(&batch.to_string()),
        Outcome::Committed { .. }
    ));
    let after = evaluator
        .evaluate(session.document(), Quality::Preview)
        .unwrap();
    let a = DVec3::from_array(before.parts[1].frames["base"].position);
    let b = DVec3::from_array(after.parts[1].frames["base"].position);
    assert!((b - a - DVec3::new(2.0, 0.0, 0.0)).length() < 1e-8);
    assert_eq!(before.parts[2].positions, after.parts[2].positions);
    assert_eq!(after.evaluated_nodes, 2);
    assert!(after.cache_hits >= 4);
}

#[test]
fn g4d_05_unknown_socket_names_attach_node_and_available_frames() {
    let session = session(&ASSEMBLY.replace("socket=\"base\"", "socket=\"missing\""));
    let error = Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .unwrap_err();
    assert_eq!(error.code, "socket.unknown");
    assert!(error.message.contains("missing"));
    assert!(error.suggestion.unwrap().contains("base"));
    assert_eq!(error.related.len(), 2);
    assert!(error.node.is_some());
}

#[test]
fn sockets_survive_material_boolean_transform_and_serialized_reload() {
    let session = session(
        "b = solid.box(size=[1,1,1])\nf = frame.add(b, name=\"mount\", position=[1,0,0])\np = solid.material(f, color=[1,0,0])\na = xform.align(p, from=[1,0,0], to=[0,1,0])\no = solid.box(size=[0.1,0.1,0.1])\nu = solid.union(a,o)\noutput u",
    );
    let doc: ve_graph::Document =
        serde_json::from_str(&session.document().canonical_json()).unwrap();
    let result = Evaluator::default()
        .evaluate(&doc, Quality::Preview)
        .unwrap();
    assert!(
        (DVec3::from_array(result.parts[0].frames["mount"].position) - DVec3::Y).length() < 1e-8
    );
    assert!(result.parts[0].material.is_some());
}

#[test]
fn duplicate_and_empty_names_are_refused_without_overwriting() {
    for (name, code) in [("mount", "socket.duplicate"), ("", "socket.name")] {
        let source = format!(
            "b = solid.box(size=[1,1,1])\nf = frame.add(b, name=\"mount\")\ng = frame.add(f, name=\"{name}\")\noutput g"
        );
        let session = session(&source);
        assert_eq!(
            Evaluator::default()
                .evaluate(session.document(), Quality::Preview)
                .unwrap_err()
                .code,
            code
        );
    }
}

#[test]
fn socket_table_gallery_places_four_legs_at_named_frames() {
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/gallery/socket_table.txt"
    ))
    .unwrap();
    let fixture = session(&source);
    let result = Evaluator::default()
        .evaluate(fixture.document(), Quality::Final)
        .unwrap();
    assert_eq!(result.parts.len(), 5);
    for (index, name) in ["leg_a", "leg_b", "leg_c", "leg_d"].into_iter().enumerate() {
        assert_eq!(result.parts[index + 1].name, name);
        assert!(result.parts[index + 1].frames.contains_key("foot"));
        let (bottom, top) = result.parts[index + 1]
            .positions
            .iter()
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(bottom, top), p| {
                (bottom.min(p[1]), top.max(p[1]))
            });
        assert!(
            (bottom + 0.76).abs() < 1e-5 && (top + 0.06).abs() < 1e-5,
            "leg vertical bounds: {bottom}..{top}"
        );
    }
}
