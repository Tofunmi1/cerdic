//! `water.fill`: a lake node that fills the hollow of a terrain tile (`ve_world::river::fill_basin`) and is drawn like river water: depth tint, wet banks, foam, no
//! square slab and no side walls. It replaces `water.surface` for lakes on a landscape.
#![expect(
    clippy::unwrap_used,
    clippy::cast_precision_loss,
    reason = "fixtures are known to be valid"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| format!("{e:?}"))?
        .commands;
    let mut session = Session::with_builtin(15);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_water_fill").unwrap(),
        label: None,
        commands,
    });
    if !matches!(applied, Outcome::Committed { .. }) {
        return Err(format!("{applied:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| format!("{e:?}"))
}

/// Flat ground at y = 0 on a 128 m tile with one round dish dug at (64, 64): pad radius 12 at -1 m, shoulder 12 m.
const DISH: &str = "t = terrain.tile@1(size=[128, 128], cell=1, seed=3, height=0)\n\
                    d = terrain.flatten@1(t, at=[64, 64], radius=12, blend=12, to=\"height\", height=-1)\n";

fn lake(extra: &str) -> String {
    format!("{DISH}w = water.fill@1(d, at=[64, 64], level=-0.3{extra})\noutput w = w\n")
}

fn water(source: &str) -> Result<Vec<[f32; 3]>, String> {
    let evaluation = evaluate(source)?;
    let part = evaluation
        .parts
        .iter()
        .find(|p| p.name == "w")
        .ok_or("no part w")?;
    Ok(part.positions.clone())
}

#[test]
fn the_lake_is_a_flat_sheet_at_the_water_level_inside_the_dish() {
    let positions = water(&lake("")).unwrap();
    assert!(positions.len() > 100, "{} vertices", positions.len());
    for p in &positions {
        assert!((p[1] - -0.3).abs() < 0.02, "vertex at height {}", p[1]);
        let r = (p[0] - 64.0).hypot(p[2] - 64.0);
        assert!(
            r < 26.0,
            "vertex {r} m from the centre: the water must stay in the dish (pad 12 + shoulder 12)"
        );
    }
}

#[test]
fn the_shore_follows_the_ground_it_is_round_not_a_square() {
    let positions = water(&lake("")).unwrap();
    let (mut min_r, mut max_r) = (f32::MAX, 0.0_f32);
    let mut far = 0.0_f32;
    for p in &positions {
        let r = (p[0] - 64.0).hypot(p[2] - 64.0);
        far = far.max(r);
        // Look only at the outermost ring of vertices.
        if r > 8.0 {
            min_r = min_r.min(r);
            max_r = max_r.max(r);
        }
    }
    // Radial symmetry: along the diagonals the water reaches as far as along the axes (a square slab would reach 1.41 times farther at its corners).
    let reach = |dx: f32, dz: f32| {
        positions
            .iter()
            .filter(|p| {
                ((p[0] - 64.0) * dz - (p[2] - 64.0) * dx).abs() < 1.0
                    && (p[0] - 64.0) * dx + (p[2] - 64.0) * dz > 0.0
            })
            .map(|p| (p[0] - 64.0).hypot(p[2] - 64.0))
            .fold(0.0, f32::max)
    };
    let axis = reach(1.0, 0.0);
    let diagonal = reach(0.707, 0.707);
    assert!(axis > 8.0 && diagonal > 8.0, "{axis} {diagonal}");
    assert!(
        (axis - diagonal).abs() < 2.0,
        "axis reach {axis}, diagonal reach {diagonal}"
    );
}

#[test]
fn the_sheet_has_no_side_walls() {
    // Every vertex of a flat sheet is at one height; a closed body would have vertices below it.
    let positions = water(&lake("")).unwrap();
    let lowest = positions.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
    let highest = positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
    assert!(highest - lowest < 0.04, "heights {lowest} to {highest}");
}

#[test]
fn a_higher_level_makes_a_bigger_lake_and_a_lower_one_a_smaller_one() {
    let extent = |level: &str| {
        let s = format!("{DISH}w = water.fill@1(d, at=[64, 64], level={level})\noutput w = w\n");
        let positions = water(&s).unwrap();
        positions
            .iter()
            .map(|p| (p[0] - 64.0).hypot(p[2] - 64.0))
            .fold(0.0, f32::max)
    };
    assert!(
        extent("-0.6") < extent("-0.3") && extent("-0.3") < extent("-0.1"),
        "{} {} {}",
        extent("-0.6"),
        extent("-0.3"),
        extent("-0.1")
    );
}

#[test]
fn only_the_dish_at_the_seed_is_filled_not_a_second_dish() {
    let source = "t = terrain.tile@1(size=[128, 128], cell=1, seed=3, height=0)\n\
                  a = terrain.flatten@1(t, at=[30, 64], radius=8, blend=8, to=\"height\", height=-1)\n\
                  b = terrain.flatten@1(a, at=[100, 64], radius=8, blend=8, to=\"height\", height=-1)\n\
                  w = water.fill@1(b, at=[30, 64], level=-0.3)\noutput w = w\n";
    let positions = water(source).unwrap();
    assert!(!positions.is_empty());
    assert!(
        positions.iter().all(|p| p[0] < 64.0),
        "water leaked into the far dish"
    );
}

#[test]
fn a_seed_on_dry_ground_is_an_error_that_names_the_seed() {
    let err = water(&format!(
        "{DISH}w = water.fill@1(d, at=[5, 5], level=-0.3)\noutput w = w\n"
    ))
    .unwrap_err();
    assert!(
        err.contains("at") || err.contains("seed") || err.contains("basin"),
        "{err}"
    );
}

#[test]
fn a_level_above_everything_floods_the_whole_tile() {
    let source = format!("{DISH}w = water.fill@1(d, at=[64, 64], level=0.5)\noutput w = w\n");
    let positions = water(&source).unwrap();
    let xs = positions.iter().map(|p| p[0]);
    assert!(
        xs.clone().fold(f32::MAX, f32::min) < 2.0 && xs.fold(f32::MIN, f32::max) > 126.0,
        "water should reach both edges of the tile"
    );
}

#[test]
fn the_look_parameters_exist_and_match_river_water() {
    let source = lake(
        ", shallow=[0.2, 0.5, 0.5], deep=[0.03, 0.2, 0.26], clarity=2.5, ripple=0.4, foam=0.5, direction=40deg, seed=4, shore=2.5, opacity=0.8",
    );
    assert!(water(&source).is_ok());
    let bad = lake(", shore=-1");
    assert!(water(&bad).is_err());
}

#[test]
fn the_lake_carries_per_vertex_depth_and_foam_like_river_water_and_uses_its_material() {
    let evaluation = evaluate(&lake("")).unwrap();
    let part = evaluation.parts.iter().find(|p| p.name == "w").unwrap();
    assert_eq!(
        part.colors.len(),
        part.positions.len(),
        "one [flow x, flow z, depth, foam] per vertex"
    );
    // No current: flow x and z are both remapped 0.5; depth in 0..1 grows toward the middle; foam is strongest at the shore.
    assert!(
        part.colors
            .iter()
            .all(|c| (c[0] - 0.5).abs() < 0.02 && (c[1] - 0.5).abs() < 0.02),
        "a lake has no flow"
    );
    let centre = part
        .positions
        .iter()
        .position(|p| (p[0] - 64.0).hypot(p[2] - 64.0) < 1.5)
        .unwrap();
    let rim = part
        .positions
        .iter()
        .enumerate()
        .filter(|(_, p)| (p[0] - 64.0).hypot(p[2] - 64.0) > 17.0)
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    assert!(!rim.is_empty());
    let rim_depth = rim.iter().map(|&i| part.colors[i][2]).fold(0.0, f32::max);
    assert!(
        part.colors[centre][2] > rim_depth,
        "deeper in the middle ({}) than at the rim ({rim_depth})",
        part.colors[centre][2]
    );
    let rim_foam = rim.iter().map(|&i| part.colors[i][3]).fold(0.0, f32::max);
    assert!(
        rim_foam > part.colors[centre][3],
        "foam gathers at the shore ({rim_foam}) not in the middle ({})",
        part.colors[centre][3]
    );
    let river = evaluate(&format!("{DISH}r = river.carve@1(d, path=\"10,10; 120,120\", width=3, depth=0.5)\nw = river.water@1(r)\noutput w = w\n")).unwrap();
    let river_part = river.parts.iter().find(|p| p.name == "w").unwrap();
    let has_water = |m: &ve_modeling::Material| {
        m.finish
            .as_ref()
            .and_then(|finish| finish.water.as_ref())
            .is_some()
    };
    let a = part.material.as_ref().map(has_water);
    let b = river_part.material.as_ref().map(has_water);
    assert_eq!(a, Some(true), "the lake is drawn with the water finish");
    assert_eq!(
        a, b,
        "a lake is drawn with the same water finish as a river"
    );
    assert!(
        part.material
            .as_ref()
            .unwrap()
            .finish
            .as_ref()
            .unwrap()
            .water
            .as_ref()
            .unwrap()
            .volumetric
    );
    assert!(
        !river_part
            .material
            .as_ref()
            .unwrap()
            .finish
            .as_ref()
            .unwrap()
            .water
            .as_ref()
            .unwrap()
            .volumetric
    );
}

#[test]
fn simulation_controls_use_the_shared_water_material() {
    let result = evaluate(&lake(
        ", sim=true, sim_resolution=64, sim_damping=0.9, sim_depth=3, rain=0.5",
    ))
    .unwrap();
    let look = result.parts[0]
        .material
        .as_ref()
        .unwrap()
        .finish
        .unwrap()
        .water
        .unwrap();
    assert!(look.sim);
    assert!(look.volumetric, "water.fill uses the integrated 3D solver");
    assert_eq!(
        (
            look.sim_resolution,
            look.sim_damping,
            look.sim_depth,
            look.rain
        ),
        (64, 0.9, 3.0, 0.5)
    );
}

#[test]
fn simulation_defaults_keep_existing_lakes_opted_out() {
    let result = evaluate(&lake("")).unwrap();
    let look = result.parts[0]
        .material
        .as_ref()
        .unwrap()
        .finish
        .unwrap()
        .water
        .unwrap();
    assert!(!look.sim);
    assert_eq!(look.sim_resolution, 128);
    assert!((look.sim_damping - 0.6).abs() < 1e-6);
    assert!((look.sim_depth - 1.0).abs() < 1e-6);
    assert!(look.rain.abs() < 1e-6);
}

#[test]
fn simulation_controls_reject_invalid_types_and_ranges() {
    for parameter in [
        "sim=1",
        "sim_resolution=31",
        "sim_resolution=257",
        "sim_resolution=32.5",
        "sim_damping=-0.1",
        "sim_damping=5.1",
        "sim_depth=0.04",
        "sim_depth=50.1",
        "rain=-0.1",
        "rain=1.1",
    ] {
        assert!(
            evaluate(&lake(&format!(", {parameter}"))).is_err(),
            "{parameter}"
        );
    }
}
