//! Light probes (`gi`) and wind on the environment nodes: authored, bounded, merged between `scene.sky` and `scene.environment`,
//! and serialized for the viewer.
#![expect(
    clippy::panic,
    clippy::unwrap_used,
    clippy::float_cmp,
    reason = "fixture setup uses exact values and reports actionable failures"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, LightProbes, Quality, SceneEnvironment, Wind};

fn evaluate(source: &str) -> Evaluation {
    let commands = parse_text(&builtin_registry(), source)
        .unwrap_or_else(|error| panic!("fixture did not parse: {error:?}"))
        .commands;
    let mut session = Session::with_builtin(312);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g12_gi_wind").unwrap(),
        label: None,
        commands,
    });
    assert!(
        matches!(applied, Outcome::Committed { .. }),
        "fixture refused: {applied:?}"
    );
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap_or_else(|error| panic!("fixture did not evaluate: {error:?}"))
}

fn environment(source: &str) -> SceneEnvironment {
    evaluate(source)
        .environment
        .unwrap_or_else(|| panic!("the document made no environment"))
}

#[test]
fn probes_are_off_unless_asked_for() {
    assert_eq!(
        environment("e = scene.environment@1()\noutput e = e").gi,
        None
    );
    assert_eq!(
        environment("e = scene.environment@1(gi=false, gi_spacing=2)\noutput e = e").gi,
        None
    );
}

#[test]
fn gi_true_carries_its_settings_with_defaults() {
    let gi = environment("e = scene.environment@1(gi=true)\noutput e = e").gi;
    assert_eq!(
        gi,
        Some(LightProbes {
            spacing: 0.0,
            bounces: 2,
            rays: 128
        })
    );
    let gi = environment("e = scene.environment@1(gi=true, gi_spacing=0.75, gi_bounces=3, gi_rays=256)\noutput e = e").gi;
    assert_eq!(
        gi,
        Some(LightProbes {
            spacing: 0.75,
            bounces: 3,
            rays: 256
        })
    );
}

#[test]
fn out_of_range_probe_settings_are_refused() {
    for bad in [
        "gi_bounces=0",
        "gi_bounces=9",
        "gi_rays=4",
        "gi_rays=9000",
        "gi_spacing=-1",
        "gi_spacing=100",
    ] {
        let source = format!("e = scene.environment@1(gi=true, {bad})\noutput e = e");
        let commands = parse_text(&builtin_registry(), &source);
        let refused = match commands {
            Err(_) => true,
            Ok(parsed) => {
                let mut session = Session::with_builtin(1);
                !matches!(
                    session.apply(&CommandBatch {
                        doc: session.document().id.clone(),
                        base_revision: 0,
                        transaction: TransactionId::parse("t_bad").unwrap(),
                        label: None,
                        commands: parsed.commands,
                    }),
                    Outcome::Committed { .. }
                )
            }
        };
        assert!(refused, "{bad} should be refused");
    }
}

#[test]
fn the_default_wind_is_a_gentle_breeze_and_authored_wind_reaches_the_result() {
    let calm = environment("e = scene.environment@1()\noutput e = e").wind;
    assert_eq!(calm, Wind::default());
    assert!(calm.strength > 0.0 && calm.strength < 1.0);
    let wind =
        environment("e = scene.environment@1(wind_direction=90, wind_strength=1.5)\noutput e = e")
            .wind;
    assert_eq!(
        wind,
        Wind {
            direction: 90.0,
            strength: 1.5
        }
    );
    let still = environment("e = scene.environment@1(wind_strength=0)\noutput e = e").wind;
    assert_eq!(still.strength, 0.0);
}

#[test]
fn a_sky_and_an_environment_merge_their_probes_and_wind_in_either_order() {
    let sky = "s = scene.sky@1(wind_direction=45, wind_strength=1, gi=true, gi_bounces=3)\noutput s = s\n";
    let ambient = "e = scene.environment@1(sky_color=[0.2, 0.3, 0.4])\noutput e = e\n";
    // Order of the outputs must not matter: the sky's wind and probes survive an environment that left its own unset.
    for source in [format!("{sky}{ambient}"), format!("{ambient}{sky}")] {
        let merged = environment(&source);
        assert!(merged.sky.is_some(), "the sky background is kept");
        assert_eq!(
            merged.sky_color,
            [0.2, 0.3, 0.4],
            "the environment's ambient wins"
        );
        assert_eq!(
            merged.wind,
            Wind {
                direction: 45.0,
                strength: 1.0
            }
        );
        assert_eq!(merged.gi.map(|gi| gi.bounces), Some(3));
    }
    // An environment that sets its own wind keeps it.
    let own = "e = scene.environment@1(wind_direction=10, wind_strength=2)\noutput e = e\n";
    for source in [format!("{sky}{own}"), format!("{own}{sky}")] {
        assert_eq!(
            environment(&source).wind,
            Wind {
                direction: 10.0,
                strength: 2.0
            }
        );
    }
}

#[test]
fn gi_and_wind_survive_a_serialization_round_trip_and_old_json_still_loads() {
    let original = environment(
        "e = scene.environment@1(gi=true, gi_rays=64, wind_direction=-30, wind_strength=0.25)\noutput e = e",
    );
    let json = serde_json::to_string(&original).unwrap();
    let back: SceneEnvironment = serde_json::from_str(&json).unwrap();
    assert_eq!(back, original);
    // An environment saved before `gi` and `wind` existed has neither field.
    let mut old: serde_json::Value = serde_json::from_str(&json).unwrap();
    let fields = old.as_object_mut().unwrap();
    assert!(fields.remove("gi").is_some() && fields.remove("wind").is_some());
    let loaded: SceneEnvironment = serde_json::from_value(old.clone()).unwrap();
    assert_eq!(loaded.gi, None);
    assert_eq!(loaded.wind, Wind::default());
    // Unknown fields stay refused.
    old.as_object_mut().unwrap().insert("fog".into(), 1.into());
    assert!(serde_json::from_value::<SceneEnvironment>(old).is_err());
}
