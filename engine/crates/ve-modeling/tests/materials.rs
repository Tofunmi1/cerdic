//! `solid.material`: a surface rides along with a solid without changing its shape.
#![expect(
    clippy::unwrap_used,
    reason = "case helpers unwrap fixtures that are known to be valid"
)]
use serde_json::{Value, json};
use ve_graph::{Outcome, Session};
use ve_modeling::{Evaluation, Evaluator, Quality};

type Row<'a> = (&'a str, &'a str, Value, &'a [&'a str]);

fn session_for(rows: &[Row<'_>]) -> Session {
    let mut session = Session::with_builtin(4);
    let mut commands = Vec::new();
    for (alias, op, params, inputs) in rows {
        commands.push(json!({"add_node": {"op": op, "params": params, "alias": alias, "label": alias.trim_start_matches('$')}}));
        for input in *inputs {
            commands.push(json!({"connect": {"from": {"node": input, "port": "out"}, "to": {"node": alias, "port": "in"}}}));
        }
    }
    let last = rows.last().unwrap().0;
    commands.push(json!({"set_output": {"name": "part", "from": {"node": last, "port": "out"}}}));
    let batch = json!({"doc": session.document().id, "base_revision": 0, "transaction": "t_mat", "commands": commands});
    assert!(matches!(
        session.apply_json(&batch.to_string()),
        Outcome::Committed { .. }
    ));
    session
}

fn evaluate(rows: &[Row<'_>]) -> Evaluation {
    Evaluator::default()
        .evaluate(session_for(rows).document(), Quality::Preview)
        .unwrap()
}

const RED: fn() -> Value = || json!({"color": [0.8, 0.1, 0.1], "metallic": 0.0, "roughness": 0.3});
const BOX: fn() -> Value = || json!({"size": [1.0, 2.0, 3.0]});

#[test]
fn a_material_keeps_the_shape_and_sets_the_surface() {
    let plain = evaluate(&[("$box", "solid.box@1", BOX(), &[])]);
    let red = evaluate(&[
        ("$box", "solid.box@1", BOX(), &[]),
        ("$red", "solid.material@1", RED(), &["$box"]),
    ]);
    assert!(
        red.parts[0]
            .positions
            .iter()
            .flatten()
            .zip(plain.parts[0].positions.iter().flatten())
            .all(|(a, b)| close(*a, *b)),
        "the shape is untouched"
    );
    assert!((red.parts[0].volume - 6.0).abs() < 1e-9);
    assert!(plain.parts[0].material.is_none());
    let material = red.parts[0].material.as_ref().unwrap();
    assert!(colour(material.color, [0.8, 0.1, 0.1]));
    assert!((material.roughness - 0.3).abs() < 1e-6);
    assert_eq!(material.opacity, 1.0);
    assert!(colour(material.emissive, [0.0, 0.0, 0.0]));
    let text = serde_json::to_value(&red.parts[0]).unwrap();
    assert!((text["material"]["color"][0].as_f64().unwrap() - 0.8).abs() < 1e-6);
}

#[test]
fn opacity_is_bounded_serialized_and_defaults_to_opaque() {
    for opacity in [0.0, 1.0] {
        let evaluated = evaluate(&[
            ("$box", "solid.box@1", BOX(), &[]),
            (
                "$glass",
                "solid.material@1",
                json!({"opacity":opacity}),
                &["$box"],
            ),
        ]);
        assert_eq!(
            evaluated.parts[0].material.as_ref().unwrap().opacity,
            opacity
        );
    }
    let translucent = evaluate(&[
        ("$box", "solid.box@1", BOX(), &[]),
        (
            "$glass",
            "solid.material@1",
            json!({"color":[0.1,0.3,0.9], "opacity":0.35}),
            &["$box"],
        ),
    ]);
    assert_eq!(
        translucent.parts[0].material.as_ref().unwrap().opacity,
        0.35
    );
    let serialized = serde_json::to_value(translucent.parts[0].material.as_ref().unwrap()).unwrap();
    assert!((serialized["opacity"].as_f64().unwrap() - 0.35).abs() < 1e-6);
    let mut old = serialized;
    old.as_object_mut().unwrap().remove("opacity");
    let restored: ve_modeling::Material = serde_json::from_value(old).unwrap();
    assert_eq!(restored.opacity, 1.0);
    for opacity in [-0.01, 1.01] {
        let mut session = Session::with_builtin(4);
        let batch = json!({"doc":session.document().id,"base_revision":0,"transaction":"bad_opacity","commands":[
            {"add_node":{"op":"solid.box@1","params":{"size":[1,1,1]},"alias":"$box"}},
            {"add_node":{"op":"solid.material@1","params":{"opacity":opacity},"alias":"$glass"}},
            {"connect":{"from":{"node":"$box","port":"out"},"to":{"node":"$glass","port":"in"}}}
        ]});
        assert!(!matches!(
            session.apply_json(&batch.to_string()),
            Outcome::Committed { .. }
        ));
    }
}

#[test]
fn a_texture_program_attaches_without_replacing_surface_values() {
    let textured = evaluate(&[
        ("$box", "solid.box@1", BOX(), &[]),
        ("$red", "solid.material@1", RED(), &["$box"]),
        (
            "$texture",
            "solid.texture@1",
            json!({"program": "albedo = fbm(1)\nroughness = constant(value=0.4)"}),
            &["$red"],
        ),
    ]);
    let material = textured.parts[0]
        .material
        .as_ref()
        .expect("texture node produces a material");
    assert!(colour(material.color, [0.8, 0.1, 0.1]));
    assert!((material.roughness - 0.3).abs() < 1e-6);
    assert_eq!(
        material.texture_program.as_deref(),
        Some("albedo = fbm(1)\nroughness = constant(value=0.4)")
    );
    let serialized = serde_json::to_value(material).unwrap();
    assert_eq!(
        serialized["texture_program"],
        "albedo = fbm(1)\nroughness = constant(value=0.4)"
    );
}

#[test]
fn a_bare_texture_program_that_paints_roughness_is_not_halved() {
    let program = |text: &str| {
        evaluate(&[
            ("$box", "solid.box@1", BOX(), &[]),
            (
                "$texture",
                "solid.texture@1",
                json!({ "program": text }),
                &["$box"],
            ),
        ])
        .parts[0]
            .material
            .clone()
            .expect("texture node produces a material")
    };
    assert!(close(
        program("albedo = fbm(1)\nroughness = fbm(2)").roughness,
        1.0
    ));
    assert!(close(program("albedo = fbm(1)").roughness, 0.5));
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-6
}

fn colour(a: [f32; 3], b: [f32; 3]) -> bool {
    a.iter().zip(&b).all(|(x, y)| close(*x, *y))
}

#[test]
fn transforms_and_booleans_keep_the_first_inputs_surface() {
    let moved = evaluate(&[
        ("$box", "solid.box@1", BOX(), &[]),
        ("$red", "solid.material@1", RED(), &["$box"]),
        (
            "$move",
            "xform.move@1",
            json!({"by": [1.0, 0.0, 0.0]}),
            &["$red"],
        ),
    ]);
    assert!(colour(
        moved.parts[0].material.as_ref().unwrap().color,
        [0.8, 0.1, 0.1]
    ));
    let cut = evaluate(&[
        ("$box", "solid.box@1", BOX(), &[]),
        ("$red", "solid.material@1", RED(), &["$box"]),
        ("$ball", "solid.sphere@1", json!({"radius": 0.5}), &[]),
        ("$cut", "solid.difference@1", json!({}), &["$red", "$ball"]),
    ]);
    assert!(
        cut.parts[0].material.is_some(),
        "the first input's surface survives a boolean"
    );
    assert!(cut.parts[0].volume < 6.0);
    let swapped = evaluate(&[
        ("$ball", "solid.sphere@1", json!({"radius": 0.5}), &[]),
        ("$box", "solid.box@1", BOX(), &[]),
        ("$red", "solid.material@1", RED(), &["$box"]),
        ("$join", "solid.union@1", json!({}), &["$ball", "$red"]),
    ]);
    assert!(
        swapped.parts[0].material.is_none(),
        "an unpainted first input gives an unpainted result"
    );
}

#[test]
fn changing_only_the_colour_reruns_only_the_material() {
    let mut session = session_for(&[
        ("$box", "solid.box@1", BOX(), &[]),
        ("$red", "solid.material@1", RED(), &["$box"]),
    ]);
    let mut evaluator = Evaluator::default();
    let first = evaluator
        .evaluate(session.document(), Quality::Preview)
        .unwrap();
    assert_eq!(first.evaluated_nodes, 2);
    let id = session.document().nodes[1].id.to_string();
    let batch = json!({"doc": session.document().id, "base_revision": session.document().revision, "transaction": "t_recolour",
        "commands": [{"set_param": {"node": id, "param": "color", "value": [0.1, 0.3, 0.9]}}]});
    assert!(matches!(
        session.apply_json(&batch.to_string()),
        Outcome::Committed { .. }
    ));
    let blue = evaluator
        .evaluate(session.document(), Quality::Preview)
        .unwrap();
    assert_eq!((blue.evaluated_nodes, blue.cache_hits), (1, 1));
    assert!(colour(
        blue.parts[0].material.as_ref().unwrap().color,
        [0.1, 0.3, 0.9]
    ));
}

#[test]
fn out_of_range_colours_are_refused_when_set() {
    let mut session = session_for(&[
        ("$box", "solid.box@1", BOX(), &[]),
        ("$red", "solid.material@1", RED(), &["$box"]),
    ]);
    let id = session.document().nodes[1].id.to_string();
    let batch = json!({"doc": session.document().id, "base_revision": session.document().revision, "transaction": "t_bad",
        "commands": [{"set_param": {"node": id, "param": "color", "value": [1.5, 0.0, 0.0]}}]});
    let Outcome::Rejected { diagnostics } = session.apply_json(&batch.to_string()) else {
        panic!("a colour channel above 1 must be refused");
    };
    assert_eq!(diagnostics[0].code, "param.range");
}
