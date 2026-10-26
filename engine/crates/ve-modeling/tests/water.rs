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

#[test]
fn lake_wave_controls_reach_the_material_without_changing_rest_geometry() {
    let scene =
        "w = water.surface(resolution=8, direction=-135, seed=73, ripple=1.7, flow=2.4)\noutput w";
    let result = evaluate(scene).unwrap();
    let part = &result.parts[0];
    let look = part
        .material
        .as_ref()
        .unwrap()
        .finish
        .unwrap()
        .water
        .unwrap();
    assert_eq!(look.direction, -135.0);
    assert_eq!(look.seed, 73);
    assert_eq!(look.ripple, 1.7);
    assert_eq!(look.flow, 2.4);
    let radians =
        evaluate(&scene.replace("direction=-135", "direction=-2.356194490192345rad")).unwrap();
    assert_eq!(
        radians.parts[0]
            .material
            .as_ref()
            .unwrap()
            .finish
            .unwrap()
            .water
            .unwrap()
            .direction,
        -135.0
    );
    let flat = evaluate(&scene.replace("ripple=1.7", "ripple=0")).unwrap();
    assert_eq!(part.positions, flat.parts[0].positions);
    assert_eq!(part.colors.len(), part.positions.len());
    assert!(
        part.colors
            .iter()
            .all(|c| c[0] == 0.5 && c[1] == 0.5 && c[2] > 0.0 && c[3] == 0.0)
    );
    let paused = evaluate(&scene.replace("flow=2.4", "flow=2.4, speed=0")).unwrap();
    assert_eq!(
        paused.parts[0]
            .material
            .as_ref()
            .unwrap()
            .finish
            .unwrap()
            .water
            .unwrap()
            .flow,
        0.0
    );
}

fn river(water: &str) -> String {
    format!(
        "t = terrain.tile(size=[32,32], cell=2, height=0)\nr = river.carve(t, path=\"0,16; 32,16\", width=6, depth=2)\nw = river.water(r{water})\noutput w"
    )
}

#[test]
fn river_wave_controls_preserve_the_current_depth_and_foam() {
    let plain = evaluate(&river("")).unwrap();
    let changed = evaluate(&river(", direction=90, seed=41, ripple=2, flow=0")).unwrap();
    assert_eq!(plain.parts[0].positions, changed.parts[0].positions);
    assert_eq!(plain.parts[0].colors, changed.parts[0].colors);
    let look = changed.parts[0]
        .material
        .as_ref()
        .unwrap()
        .finish
        .unwrap()
        .water
        .unwrap();
    assert_eq!(
        (look.direction, look.seed, look.ripple, look.flow),
        (90.0, 41, 2.0, 0.0)
    );
    let default = plain.parts[0]
        .material
        .as_ref()
        .unwrap()
        .finish
        .unwrap()
        .water
        .unwrap();
    assert_eq!(
        (
            default.direction,
            default.seed,
            default.ripple,
            default.flow
        ),
        (0.0, 0, 0.5, 1.0)
    );
    let mut old = serde_json::to_value(default).unwrap();
    old.as_object_mut().unwrap().remove("direction");
    old.as_object_mut().unwrap().remove("seed");
    assert_eq!(default, serde_json::from_value(old).unwrap());
    assert_eq!(
        look,
        serde_json::from_value(serde_json::to_value(look).unwrap()).unwrap()
    );
}

#[test]
fn wave_controls_have_defaults_and_reject_out_of_range_values_on_both_nodes() {
    let lake = evaluate("w = water.surface(resolution=4)\noutput w").unwrap();
    let look = lake.parts[0]
        .material
        .as_ref()
        .unwrap()
        .finish
        .unwrap()
        .water
        .unwrap();
    assert_eq!(
        (look.direction, look.seed, look.ripple, look.flow),
        (0.0, 0, 1.0, 1.0)
    );
    for param in [
        "direction=361",
        "direction=-361",
        "seed=-1",
        "seed=1000001",
        "ripple=-0.1",
        "ripple=2.1",
        "flow=-0.1",
        "flow=4.1",
    ] {
        let scene = format!("w = water.surface({param})\noutput w");
        assert!(parse_text(&builtin_registry(), &scene).is_err(), "{scene}");
        let scene = river(&format!(", {param}"));
        assert!(parse_text(&builtin_registry(), &scene).is_err(), "{scene}");
    }
}

#[test]
fn simulation_controls_and_defaults_reach_the_part() {
    let look = |text: &str| {
        evaluate(text).unwrap().parts[0]
            .material
            .as_ref()
            .unwrap()
            .finish
            .unwrap()
            .water
            .unwrap()
    };
    let default = look(LAKE);
    assert!(!default.sim);
    assert_eq!(default.sim_resolution, 128);
    assert_eq!(default.sim_damping, 0.6);
    assert_eq!(default.sim_depth, 1.0);
    assert_eq!(default.rain, 0.0);
    let sim = look(&LAKE.replace(
        "resolution=24",
        "resolution=24, sim=true, sim_resolution=96, sim_damping=1.2, sim_depth=2.5, rain=0.8",
    ));
    assert!(sim.sim);
    assert_eq!(
        (sim.sim_resolution, sim.sim_damping, sim.sim_depth, sim.rain),
        (96, 1.2, 2.5, 0.8)
    );
    for param in [
        "sim=1",
        "sim_resolution=31",
        "sim_resolution=257",
        "sim_damping=-1",
        "sim_damping=6",
        "sim_depth=0",
        "sim_depth=51",
        "rain=1.1",
    ] {
        assert!(
            parse_text(&builtin_registry(), &format!("w = water.surface({param})")).is_err(),
            "{param}"
        );
    }
}
