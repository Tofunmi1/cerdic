//! B3: the authored scene environment is validated, evaluated and serialized.
#![expect(
    clippy::panic,
    clippy::unwrap_used,
    clippy::float_cmp,
    reason = "fixture setup uses exact values and reports actionable failures"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn apply(source: &str, seed: u64) -> (Session, Outcome) {
    let commands = parse_text(&builtin_registry(), source)
        .unwrap_or_else(|error| panic!("environment fixture did not parse: {error:?}"))
        .commands;
    let mut session = Session::with_builtin(seed);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g9_environment").unwrap(),
        label: None,
        commands,
    });
    (session, applied)
}

fn evaluate(source: &str) -> Evaluation {
    let (session, applied) = apply(source, 209);
    assert!(
        matches!(applied, Outcome::Committed { .. }),
        "fixture refused: {applied:?}"
    );
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap_or_else(|error| panic!("environment fixture did not evaluate: {error:?}"))
}

#[test]
fn default_environment_is_a_cool_sky_over_a_warm_ground() {
    let result = evaluate("environment = scene.environment@1()");
    assert!(result.parts.is_empty());
    let environment = result.environment.expect("a default environment evaluates");
    assert!((environment.intensity - 0.15).abs() < f32::EPSILON);
    assert!(environment.bounce.abs() < f32::EPSILON);
    assert!(environment.reflection.abs() < f32::EPSILON);
    // Blue sky, warm ground, as in `VeAmbientLight`'s defaults.
    assert!(environment.sky_color[2] > environment.sky_color[0]);
    assert!(environment.ground_color[0] > environment.ground_color[2]);
}

#[test]
fn explicit_colours_and_bounce_reach_the_result() {
    let result = evaluate(
        "environment = scene.environment@1(sky_color=[0.1, 0.2, 0.4], ground_color=[0.5, 0.3, 0.2], \
         intensity=2, reflection=0.5, bounce=0.8, bounce_color=[0.7, 0.5, 0.3])",
    );
    let environment = result.environment.expect("an environment evaluates");
    assert_eq!(environment.sky_color, [0.1, 0.2, 0.4]);
    assert_eq!(environment.ground_color, [0.5, 0.3, 0.2]);
    assert_eq!(environment.bounce_color, [0.7, 0.5, 0.3]);
    assert!((environment.intensity - 2.0).abs() < f32::EPSILON);
    assert!((environment.reflection - 0.5).abs() < f32::EPSILON);
    assert!((environment.bounce - 0.8).abs() < f32::EPSILON);
}

#[test]
fn the_environment_survives_a_serialization_round_trip() {
    let original = evaluate("environment = scene.environment@1(bounce=0.4, reflection=0.25)")
        .environment
        .expect("an environment evaluates");
    let json = serde_json::to_string(&original).unwrap();
    let restored: ve_modeling::SceneEnvironment = serde_json::from_str(&json).unwrap();
    assert_eq!(original, restored);
}

#[test]
fn duplicate_environments_are_refused() {
    let (session, applied) = apply(
        "first = scene.environment@1()\nsecond = scene.environment@1(bounce=0.5)",
        211,
    );
    assert!(matches!(applied, Outcome::Committed { .. }));
    let error = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap_err();
    assert_eq!(error.code, "scene.environment.duplicate");
}

#[test]
fn out_of_range_environment_is_refused_before_evaluation() {
    let error = parse_text(
        &builtin_registry(),
        "environment = scene.environment@1(intensity=99)",
    )
    .expect_err("intensity above the declared maximum is refused");
    assert_eq!(error[0].code, "param.range");
}
