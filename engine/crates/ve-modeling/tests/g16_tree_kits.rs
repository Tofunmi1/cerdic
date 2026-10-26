//! Plant kits (written in the language, no engine code knows a species): `oak`, `pine` and `grass_tuft` evaluate cleanly, stand on the
//! ground at their documented height, stay within a triangle budget that lets a forest of them be placed, and their exposed parameters work.
//! Written before the kits; do not weaken.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "fixture failures include the model diagnostic rather than hiding it"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Kit, PLANT_KITS, Quality};

fn kit(name: &str) -> &'static Kit {
    PLANT_KITS
        .iter()
        .find(|kit| kit.name == name)
        .unwrap_or_else(|| panic!("no kit named {name}"))
}

fn run(name: &str, args: &str) -> Evaluation {
    let source = format!("{}\nitem = {name}({args})\noutput item", kit(name).source);
    let parsed = parse_text(&builtin_registry(), &source)
        .unwrap_or_else(|errors| panic!("{name} parse errors: {errors:?}"));
    let mut session = Session::with_builtin(1601);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g16_tree_kits").unwrap(),
        label: None,
        commands: parsed.commands,
    });
    assert!(
        matches!(outcome, Outcome::Committed { .. }),
        "{name}: {outcome:?}"
    );
    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap_or_else(|error| {
            panic!(
                "{name}: {}: {} (node {:?})",
                error.code, error.message, error.node
            )
        });
    assert!(result.warnings.is_empty(), "{name}: {:?}", result.warnings);
    assert!(!result.parts.is_empty(), "{name} produced nothing");
    result
}

fn extent(result: &Evaluation) -> ([f32; 3], [f32; 3]) {
    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for p in result.parts.iter().flat_map(|part| &part.positions) {
        for axis in 0..3 {
            lo[axis] = lo[axis].min(p[axis]);
            hi[axis] = hi[axis].max(p[axis]);
        }
    }
    (lo, hi)
}

fn triangles(result: &Evaluation) -> usize {
    result.parts.iter().map(|p| p.indices.len() / 3).sum()
}

/// (name, height range in metres, widest the crown may be, triangle ceiling)
const PLANTS: [(&str, (f32, f32), f32, usize); 3] = [
    ("oak", (8.0, 12.0), 14.0, 12_000),
    ("pine", (9.0, 14.0), 8.0, 6_000),
    ("grass_tuft", (0.3, 0.7), 0.6, 300),
];

#[test]
fn every_plant_kit_exists_and_documents_its_size() {
    for (name, ..) in PLANTS {
        let doc = kit(name).doc;
        assert!(
            doc.contains(" m"),
            "{name}: the doc gives the footprint in metres: {doc}"
        );
    }
}

#[test]
fn plants_stand_on_the_ground_at_their_documented_size_within_budget() {
    for (name, (low, high), widest, ceiling) in PLANTS {
        let result = run(name, "");
        let (lo, hi) = extent(&result);
        assert!(
            lo[1].abs() < 0.05,
            "{name} stands on y = 0, lowest point {}",
            lo[1]
        );
        assert!(
            (low..=high).contains(&hi[1]),
            "{name} is {low} to {high} m tall, not {}",
            hi[1]
        );
        let width = (hi[0] - lo[0]).max(hi[2] - lo[2]);
        assert!(
            width <= widest,
            "{name} is at most {widest} m wide, not {width}"
        );
        assert!(width > 0.05, "{name} has width");
        assert!(
            triangles(&result) <= ceiling,
            "{name} has {} triangles, over {ceiling}",
            triangles(&result)
        );
        assert!(triangles(&result) > 20, "{name} is more than a stub");
    }
}

#[test]
fn plants_have_more_than_one_colour_where_they_should() {
    // A tree is bark and leaves; the evaluation reports the surface colours as separate regions or parts.
    for name in ["oak", "pine"] {
        let result = run(name, "");
        // One fused solid with several surfaces comes out as one part per material region.
        let regions: std::collections::BTreeSet<u32> =
            result.parts.iter().map(|p| p.region).collect();
        assert!(
            regions.len() >= 2,
            "{name} has bark and leaves, found {} surface(s)",
            regions.len()
        );
    }
}

#[test]
fn plants_are_deterministic() {
    for (name, ..) in PLANTS {
        let a = run(name, "");
        let b = run(name, "");
        assert_eq!(a.parts.len(), b.parts.len());
        assert_eq!(
            a.parts[0].positions, b.parts[0].positions,
            "{name} gives the same shape every time"
        );
    }
}

#[test]
fn a_detail_parameter_trades_triangles_for_smoothness_so_a_scene_can_fit_its_budget() {
    let fine = run("oak", "");
    let coarse = run("oak", "edge_length=0.8");
    assert!(
        triangles(&coarse) * 3 < triangles(&fine),
        "a coarser oak has far fewer triangles: {} against {}",
        triangles(&coarse),
        triangles(&fine)
    );
    let (lo, hi) = extent(&coarse);
    assert!(
        lo[1].abs() < 0.05 && (8.0..=12.0).contains(&hi[1]),
        "and is still an oak: {lo:?} {hi:?}"
    );
    let pine = run("pine", "");
    let lite = run("pine", "ragged=0.0");
    assert!(
        triangles(&lite) * 4 < triangles(&pine),
        "a smooth-outlined pine needs far fewer triangles: {} against {}",
        triangles(&lite),
        triangles(&pine)
    );
}

#[test]
fn the_exposed_parameters_change_the_plant() {
    let plain = run("oak", "");
    let seeded = run("oak", "seed=11");
    assert_ne!(
        plain.parts[0].positions, seeded.parts[0].positions,
        "another seed is another oak"
    );
    let (_, plain_top) = extent(&plain);
    let tall = run("pine", "");
    let short = run("pine", "scale=[0.6, 0.6, 0.6]");
    assert!(
        extent(&short).1[1] < extent(&tall).1[1] * 0.7,
        "scale shortens a pine: {} against {}",
        extent(&short).1[1],
        extent(&tall).1[1]
    );
    assert!(plain_top[1] > 0.0);
}

#[test]
fn grass_kit_keeps_its_geometry_and_exposes_its_look() {
    let plain = run("grass_tuft", "");
    assert_eq!(triangles(&plain), 42);
    let f = plain.parts[0].material.as_ref().unwrap().foliage.unwrap();
    assert!(f.grass);
    assert_eq!(
        (f.ground_blend, f.fade_near, f.fade_far, f.translucency),
        (0.6, 25.0, 70.0, 0.5)
    );
    let changed = run(
        "grass_tuft",
        "color=[0.2,0.4,0.1], ground_color=[0.3,0.2,0.1], ground_blend=0.8, fade_near=10, fade_far=40, translucency=0.7, exposure_boost=1.1",
    );
    assert_eq!(triangles(&changed), 42);
    assert_eq!(plain.parts[0].positions, changed.parts[0].positions);
    let material = changed.parts[0].material.as_ref().unwrap();
    assert_eq!(material.color, [0.2, 0.4, 0.1]);
    let f = material.foliage.unwrap();
    assert_eq!(f.ground_color, [0.3, 0.2, 0.1]);
    assert_eq!(
        (
            f.ground_blend,
            f.fade_near,
            f.fade_far,
            f.translucency,
            f.exposure_boost
        ),
        (0.8, 10.0, 40.0, 0.7, 1.1)
    );
}
