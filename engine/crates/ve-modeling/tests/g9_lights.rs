//! B1: authored scene lights are validated, evaluated and kept separate from mesh parts.
#![expect(
    clippy::panic,
    clippy::unwrap_used,
    clippy::float_cmp,
    reason = "fixture setup uses exact values and reports actionable failures"
)]
use std::fmt::Write as _;
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality, SceneLightKind};

fn evaluate(source: &str) -> ve_modeling::Evaluation {
    let commands = parse_text(&builtin_registry(), source)
        .unwrap_or_else(|error| panic!("light fixture did not parse: {error:?}"))
        .commands;
    let mut session = Session::with_builtin(109);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g9_lights").unwrap(),
        label: None,
        commands,
    });
    assert!(
        matches!(applied, Outcome::Committed { .. }),
        "fixture refused: {applied:?}"
    );
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap_or_else(|error| panic!("light fixture did not evaluate: {error:?}"))
}

#[test]
fn point_spot_and_sun_outputs_evaluate_without_geometry_side_effects() {
    let result = evaluate(
        "bulb = light.point@1(position=[1, 2, 3], color=[1, 0.5, 0.2], intensity=20, range=9)\n\
         cone = light.spot@1(position=[0, 3, 0], direction=[0, -1, 0], color=[1, 0.8, 0.6], inner_angle=0.4, outer_angle=0.8)\n\
         sun = light.sun@1(direction=[-1, -1, -1], color=[1, 0.95, 0.85], intensity=4)\n\
         cube = solid.box@1(size=[1, 1, 1])",
    );
    assert_eq!(result.parts.len(), 1);
    assert_eq!(result.lights.len(), 3);
    assert_eq!(result.lights[0].kind, SceneLightKind::Point);
    assert_eq!(result.lights[0].position, [1.0, 2.0, 3.0]);
    assert_eq!(result.lights[0].range, 9.0);
    assert_eq!(result.lights[1].kind, SceneLightKind::Spot);
    assert_eq!(result.lights[1].direction, [0.0, -1.0, 0.0]);
    assert!((result.lights[1].inner_angle - 0.4_f32.to_radians()).abs() < f32::EPSILON);
    assert_eq!(result.lights[2].kind, SceneLightKind::Sun);
    assert!(
        (result.lights[2]
            .direction
            .iter()
            .map(|v| v * v)
            .sum::<f32>()
            .sqrt()
            - 1.0)
            .abs()
            < 1e-6
    );
}

#[test]
fn lights_can_be_the_only_implicit_scene_outputs() {
    let result = evaluate("lamp = light.point@1(position=[0, 2, 0], color=[1, 1, 1])");
    assert!(result.parts.is_empty());
    assert_eq!(result.lights.len(), 1);
    assert_eq!(result.lights[0].name, "lamp");
}

#[test]
fn emissive_material_opt_in_generates_one_area_scaled_point_light() {
    let result = evaluate(
        "box = solid.box@1(size=[2, 1, 1])\n\
         glow = solid.material@1(box, emissive=[4, 2, 1], emits_light=true)\n\
         output glow",
    );
    assert_eq!(result.parts.len(), 1);
    assert_eq!(result.lights.len(), 1);
    assert_eq!(result.lights[0].kind, SceneLightKind::Point);
    assert_eq!(result.lights[0].position, [0.0, 0.0, 0.0]);
    assert!((result.lights[0].intensity - 20.0).abs() < f32::EPSILON);

    let unmarked = evaluate(
        "box = solid.box@1(size=[2, 1, 1])\n\
         glow = solid.material@1(box, emissive=[4, 2, 1])\n\
         output glow",
    );
    assert!(unmarked.lights.is_empty());
}

#[test]
fn emissive_lights_share_the_scene_light_budget() {
    let mut source = String::new();
    let mut materials = Vec::new();
    for index in 0..=ve_modeling::MAX_SCENE_LIGHTS {
        let shade = format!("0.{index:03}");
        let _ = writeln!(
            source,
            "box_{index} = solid.box@1(size=[1, 1, 1])\n\
             placed_{index} = xform.move@1(box_{index}, by=[{}, 0, 0])\n\
             glow_{index} = solid.material@1(placed_{index}, color=[{shade}, 0.2, 0.3], emissive=[4, 2, 1], emits_light=true)",
            index * 2
        );
        materials.push(format!("glow_{index}"));
    }
    let groups: Vec<String> = materials
        .chunks(43)
        .enumerate()
        .map(|(group, inputs)| {
            let name = format!("joined_{group}");
            let _ = writeln!(source, "{name} = solid.union@1({})", inputs.join(", "));
            name
        })
        .collect();
    let _ = writeln!(
        source,
        "scene = solid.union@1({})\noutput scene",
        groups.join(", ")
    );
    let commands = parse_text(&builtin_registry(), &source).unwrap().commands;
    let mut session = Session::with_builtin(111);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g9_light_cap").unwrap(),
        label: None,
        commands,
    });
    assert!(
        matches!(applied, Outcome::Committed { .. }),
        "fixture refused: {applied:?}"
    );
    let error = Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .unwrap_err();
    assert_eq!(error.code, "budget.lights");
}

#[test]
fn invalid_light_direction_and_cone_are_refused_with_specific_codes() {
    for (source, code) in [
        (
            "sun = light.sun@1(direction=[0, 0, 0], color=[1, 1, 1])",
            "light.zero_direction",
        ),
        (
            "spot = light.spot@1(position=[0, 1, 0], direction=[0, 1, 0], color=[1, 1, 1], inner_angle=0.8, outer_angle=0.4)",
            "light.invalid_cone",
        ),
    ] {
        let commands = parse_text(&builtin_registry(), source).unwrap().commands;
        let mut session = Session::with_builtin(110);
        let applied = session.apply(&CommandBatch {
            doc: session.document().id.clone(),
            base_revision: 0,
            transaction: TransactionId::parse("t_g9_bad_light").unwrap(),
            label: None,
            commands,
        });
        assert!(matches!(applied, Outcome::Committed { .. }));
        let error = Evaluator::default()
            .evaluate(session.document(), Quality::Preview)
            .unwrap_err();
        assert_eq!(error.code, code);
    }
}
