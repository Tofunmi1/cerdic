#![expect(
    clippy::unwrap_used,
    reason = "valid graph fixtures should fail directly"
)]
#![expect(missing_docs, reason = "integration test names are the specification")]

use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn evaluate(source: &str) -> Result<ve_modeling::Evaluation, String> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|error| format!("{error:?}"))?
        .commands;
    let mut session = Session::with_builtin(79);
    let result = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_streetscape").unwrap(),
        label: None,
        commands,
    });
    if !matches!(result, Outcome::Committed { .. }) {
        return Err(format!("{result:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|error| format!("{}: {}", error.code, error.message))
}

#[test]
fn road_kit_parts_have_uvs_and_cardinal_placement() {
    for part in ["asphalt", "markings", "crosswalk", "kerb", "pavement"] {
        let source = format!(
            "road = street.road@1(at=[4,0,9], length=40, width=12, lanes=4, axis=\"z\", part=\"{part}\")\noutput road = road\n"
        );
        let result = evaluate(&source).unwrap();
        let mesh = &result.parts[0];
        assert!(!mesh.indices.is_empty(), "empty road part {part}");
        assert_eq!(mesh.uvs.len(), mesh.indices.len());
        assert!(mesh.positions.iter().flatten().all(|v| v.is_finite()));
        assert!(mesh.positions.iter().all(|p| p[0] > 0.0 || p[2] > -12.0));
    }
}

#[test]
fn every_street_prop_builds_body_and_accents() {
    for kind in [
        "mast",
        "streetlight",
        "traffic_signal",
        "sign",
        "utility_mast",
        "bollard",
        "hydrant",
        "bench",
        "bin",
        "bus_shelter",
    ] {
        for part in ["body", "accent"] {
            let source = format!(
                "prop = street.prop@1(at=[3,0,-2], height=7, axis=\"x\", kind=\"{kind}\", part=\"{part}\")\noutput prop = prop\n"
            );
            let result = evaluate(&source).unwrap_or_else(|e| panic!("{kind}/{part}: {e}"));
            let mesh = &result.parts[0];
            assert!(!mesh.indices.is_empty(), "empty {kind}/{part}");
            assert_eq!(mesh.uvs.len(), mesh.indices.len());
            assert!(mesh.positions.iter().flatten().all(|v| v.is_finite()));
        }
    }
    for kind in ["streetlight", "traffic_signal"] {
        let source =
            format!("prop = street.prop@1(kind=\"{kind}\", part=\"lens\")\noutput prop = prop\n");
        assert!(!evaluate(&source).unwrap().parts[0].indices.is_empty());
    }
}

#[test]
fn unknown_props_and_invalid_road_dimensions_refuse() {
    for source in [
        "prop = street.prop@1(kind=\"spaceship\")\noutput prop = prop",
        "road = street.road@1(length=4)\noutput road = road",
        "prop = street.prop@1(kind=\"bench\", part=\"lens\")\noutput prop = prop",
    ] {
        assert!(evaluate(source).is_err(), "unexpectedly accepted {source}");
    }
}
