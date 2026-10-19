//! Water geometry is a deterministic, budgeted Solid with a transparent water material.
#![expect(
    clippy::unwrap_used,
    reason = "the test fixture is a valid bounded scene"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(text: &str) -> Result<Evaluation, String> {
    let batch = parse_text(&builtin_registry(), text).map_err(|e| format!("{e:?}"))?;
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_water").unwrap(),
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

const LAKE: &str = "water = water.surface@1(size=[8,6], level=1, depth=1, wave_height=0.08, wave_length=2, seed=7, resolution=24)\noutput water = water";

#[test]
fn same_seed_repeats_exactly_and_carries_the_water_material() {
    let a = evaluate(LAKE).unwrap();
    let b = evaluate(LAKE).unwrap();
    assert_eq!(a.parts[0].positions, b.parts[0].positions);
    assert_eq!(a.parts[0].indices, b.parts[0].indices);
    let material = a.parts[0].material.as_ref().unwrap();
    assert_eq!(material.recipe, ve_modeling::MaterialRecipe::Water);
    assert_eq!(material.opacity.to_bits(), 0.6_f32.to_bits());
    assert_eq!(a.parts[0].indices.len() / 3, 2 * 24 * 24 + 12 * 24);
}

#[test]
fn resolution_is_bounded_and_material_can_be_overridden() {
    assert!(
        parse_text(
            &builtin_registry(),
            &LAKE.replace("resolution=24", "resolution=256")
        )
        .is_err()
    );
    let override_scene = "water = water.surface@1(resolution=8)\npaint = solid.material(water, color=[0.8,0.8,0.9], roughness=0.8, metallic=0, emissive=[0,0,0])\noutput water = paint";
    let painted = evaluate(override_scene).unwrap();
    assert_eq!(
        painted.parts[0].material.as_ref().unwrap().recipe,
        ve_modeling::MaterialRecipe::None
    );
}

#[test]
fn invalid_wave_and_depth_values_are_rejected_at_the_graph_boundary() {
    for source in [
        "water = water.surface@1(depth=0)",
        "water = water.surface@1(wave_length=0)",
        "water = water.surface@1(opacity=1.2)",
    ] {
        assert!(parse_text(&builtin_registry(), source).is_err(), "{source}");
    }
}
