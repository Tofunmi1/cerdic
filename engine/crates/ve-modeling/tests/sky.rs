//! Procedural sky output coexists with an authored ambient environment and contributes a sun.
#![expect(clippy::unwrap_used, reason = "fixtures are known-valid scenes")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(text: &str) -> Result<Evaluation, String> {
    let batch = parse_text(&builtin_registry(), text).map_err(|e| format!("{e:?}"))?;
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_sky").unwrap(),
        label: None,
        commands: batch.commands,
    });
    if matches!(outcome, Outcome::Rejected { .. }) {
        return Err(format!("{outcome:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .map_err(|e| format!("{}: {}", e.code, e.message))
}

#[test]
fn sky_is_deterministic_and_emits_ambient_and_sun_data() {
    let text = "sky = scene.sky@1(sun_elevation=12, sun_azimuth=45, turbidity=8, clouds=true, cloud_cover=0.7, seed=11)\noutput sky";
    let first = evaluate(text).unwrap();
    let second = evaluate(text).unwrap();
    let sky = first.environment.as_ref().unwrap().sky.as_ref().unwrap();
    assert_eq!(
        sky,
        second.environment.as_ref().unwrap().sky.as_ref().unwrap()
    );
    assert_eq!(first.lights.len(), 1);
    assert_eq!(first.lights[0].kind, ve_modeling::SceneLightKind::Sun);
    assert!(first.lights[0].direction[1] < 0.0);
    assert!(sky.clouds && sky.cloud_cover > 0.6);
}

#[test]
fn the_sun_is_warm_light_not_the_blue_of_the_sky() {
    let colour = |elevation: u32| {
        let text =
            format!("sky = scene.sky@1(sun_elevation={elevation}, sun_azimuth=30)\noutput sky");
        evaluate(&text).unwrap().lights[0].color
    };
    let (noon, dusk) = (colour(60), colour(4));
    assert!(
        noon[0] >= noon[2] && noon[0] > 0.9,
        "a high sun is warm white: {noon:?}"
    );
    assert!(
        dusk[0] > dusk[1] && dusk[1] > dusk[2],
        "a low sun is orange: {dusk:?}"
    );
    assert!(dusk[2] < noon[2], "the sun reddens as it sets");
}

#[test]
fn sky_and_environment_coexist_but_duplicate_skies_are_refused() {
    let both = evaluate("sky = scene.sky@1(sun_elevation=55)\noutput sky\nenv = scene.environment@1(sky_color=[0.8,0.7,0.5], ground_color=[0.2,0.2,0.2])\noutput env").unwrap();
    let scene = both.environment.unwrap();
    assert_eq!(scene.sky_color, [0.8, 0.7, 0.5]);
    assert!(scene.sky.is_some());
    let error =
        evaluate("a = scene.sky@1()\noutput a\nb = scene.sky@1(seed=2)\noutput b").unwrap_err();
    assert!(error.contains("scene.sky.duplicate"), "{error}");
}

#[test]
fn sky_parameter_ranges_are_enforced() {
    for text in [
        "scene.sky@1(turbidity=0)",
        "scene.sky@1(sun_elevation=91)",
        "scene.sky@1(cloud_cover=1.1)",
    ] {
        assert!(parse_text(&builtin_registry(), text).is_err(), "{text}");
    }
}
