//! `haze` and `light_shafts` on `scene.environment` and `scene.sky`: distance haze (fog) and light shafts through it, evaluated, validated,
//! serialized and reaching the result. The renderer's mapping onto Bevy's fog is a separate step; this is the language and the data.
#![expect(
    clippy::panic,
    clippy::unwrap_used,
    clippy::float_cmp,
    reason = "fixture setup uses exact values and reports actionable failures"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn apply(source: &str) -> (Session, Outcome) {
    let commands = parse_text(&builtin_registry(), source)
        .unwrap_or_else(|error| panic!("haze fixture did not parse: {error:?}"))
        .commands;
    let mut session = Session::with_builtin(212);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g12_haze").unwrap(),
        label: None,
        commands,
    });
    (session, applied)
}

fn evaluate(source: &str) -> Evaluation {
    let (session, applied) = apply(source);
    assert!(
        matches!(applied, Outcome::Committed { .. }),
        "fixture refused: {applied:?}"
    );
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap_or_else(|error| panic!("haze fixture did not evaluate: {error:?}"))
}

#[test]
fn an_environment_without_haze_has_none() {
    let environment = evaluate("environment = scene.environment@1()")
        .environment
        .unwrap();
    assert_eq!(environment.haze.density, 0.0);
    assert_eq!(environment.haze.shafts, 0.0);
}

#[test]
fn haze_settings_on_an_environment_reach_the_result() {
    let environment = evaluate(
        "environment = scene.environment@1(haze=0.4, haze_color=[0.6, 0.7, 0.9], haze_height=12, light_shafts=0.7)",
    )
    .environment
    .unwrap();
    assert!((environment.haze.density - 0.4).abs() < 1e-6);
    assert_eq!(environment.haze.color, [0.6, 0.7, 0.9]);
    assert!((environment.haze.height - 12.0).abs() < 1e-6);
    assert!((environment.haze.shafts - 0.7).abs() < 1e-6);
}

#[test]
fn haze_settings_on_a_sky_reach_the_result_too() {
    let environment = evaluate("sky = scene.sky@1(sun_elevation=20, haze=0.25, light_shafts=0.5)")
        .environment
        .unwrap();
    assert!((environment.haze.density - 0.25).abs() < 1e-6);
    assert!((environment.haze.shafts - 0.5).abs() < 1e-6);
    // The defaults for what it leaves out: a pale blue-grey haze, uniform with height.
    assert!(environment.haze.color[2] >= environment.haze.color[0]);
    assert_eq!(environment.haze.height, 0.0);
}

#[test]
fn haze_survives_a_serialization_round_trip() {
    let original = evaluate("environment = scene.environment@1(haze=0.3, light_shafts=0.2)")
        .environment
        .unwrap();
    let json = serde_json::to_string(&original).unwrap();
    let back: ve_modeling::SceneEnvironment = serde_json::from_str(&json).unwrap();
    assert_eq!(original, back);
}

#[test]
fn an_older_saved_environment_without_haze_still_loads_as_clear_air() {
    let original = evaluate("environment = scene.environment@1()")
        .environment
        .unwrap();
    let mut value = serde_json::to_value(&original).unwrap();
    value.as_object_mut().unwrap().remove("haze");
    let back: ve_modeling::SceneEnvironment = serde_json::from_value(value).unwrap();
    assert_eq!(back.haze.density, 0.0);
}

#[test]
fn out_of_range_haze_is_refused_with_the_parameter_named() {
    for source in [
        "environment = scene.environment@1(haze=1.5)",
        "environment = scene.environment@1(haze=-0.1)",
        "environment = scene.environment@1(light_shafts=2)",
        "environment = scene.environment@1(haze_height=-3)",
    ] {
        // The text language refuses an out-of-range literal while parsing, before any command exists.
        let refused = parse_text(&builtin_registry(), source)
            .expect_err("an out-of-range haze must not parse");
        let text = format!("{refused:?}");
        assert!(text.contains("param.range"), "{source}: {text}");
    }
}

#[test]
fn the_operator_documents_the_new_parameters_for_a_model() {
    let text = format!("{:?}", builtin_registry());
    for parameter in ["haze", "haze_color", "haze_height", "light_shafts"] {
        assert!(text.contains(parameter), "{parameter} is documented");
    }
}
