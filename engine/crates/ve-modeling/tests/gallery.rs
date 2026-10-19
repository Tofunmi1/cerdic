//! G3-23: the three gallery programs read, evaluate and match their analytic volumes. The browser
//! test `web/tests/gallery.mjs` loads the same files and reviews them from the fixed views.
#![expect(
    clippy::unwrap_used,
    reason = "the helper unwraps fixtures that are known to be valid"
)]
use std::path::Path;
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn volume(name: &str) -> f64 {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/gallery/{name}.txt"));
    let text = std::fs::read_to_string(path).unwrap();
    let batch = parse_text(&builtin_registry(), &text).unwrap();
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_gallery").unwrap(),
        label: None,
        commands: batch.commands,
    });
    assert!(
        matches!(outcome, Outcome::Committed { .. }),
        "{name}: {outcome:?}"
    );
    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap();
    assert_eq!(result.parts.len(), 1, "{name}");
    result.parts[0].volume
}

fn evaluated(name: &str) -> ve_modeling::Evaluation {
    evaluated_at(name, Quality::Final)
}

fn evaluated_at(name: &str, quality: Quality) -> ve_modeling::Evaluation {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/gallery/{name}.txt"));
    let text = std::fs::read_to_string(path).unwrap();
    let batch = parse_text(&builtin_registry(), &text).unwrap();
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_gallery_models").unwrap(),
        label: None,
        commands: batch.commands,
    });
    assert!(
        matches!(outcome, Outcome::Committed { .. }),
        "{name}: {outcome:?}"
    );
    Evaluator::default()
        .evaluate(session.document(), quality)
        .unwrap()
}

fn near(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance * b
}

fn part_bounds(part: &ve_modeling::Part) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for point in &part.positions {
        for axis in 0..3 {
            min[axis] = min[axis].min(point[axis]);
            max[axis] = max[axis].max(point[axis]);
        }
    }
    (min, max)
}

#[test]
fn the_vase_is_a_hollow_solid_of_the_expected_volume() {
    let v = volume("vase");
    assert!(v > 5.0e-4 && v < 1.2e-3, "{v}");
}

#[test]
fn the_ceramic_vase_is_a_smooth_hollow_solid_with_a_surface() {
    let v = volume("ceramic_vase");
    assert!(v > 1.0e-3 && v < 4.0e-3, "{v}");
}

#[test]
fn g5_gallery_surface_programs_evaluate_as_solids() {
    for name in [
        "pipe_elbow",
        "lofted_vase",
        "curved_panel",
        "coil_spring",
        "turbine_blade",
    ] {
        let result = volume(name);
        assert!(result.is_finite() && result > 0.0, "{name}: {result}");
    }
}

#[test]
fn ambitious_gallery_programs_parse_and_evaluate() {
    for name in [
        "staircase",
        "lamp",
        "goblet",
        "bridge",
        "sword",
        "car",
        "vase_smooth",
        "car_fused",
    ] {
        let result = evaluated(name);
        assert!(!result.parts.is_empty(), "{name}: no output parts");
        assert!(
            result
                .parts
                .iter()
                .all(|part| part.volume.is_finite() && part.volume > 0.0),
            "{name}: invalid part volume"
        );
    }
}

#[test]
fn g6_gallery_polygon_programs_evaluate_with_their_analytic_volumes() {
    // The facade is an open sheet: it draws, but encloses nothing.
    let facade = evaluated("poly_facade");
    assert_eq!(facade.parts.len(), 1);
    assert!(facade.parts[0].volume.abs() < 1.0e-9 && !facade.parts[0].indices.is_empty());
    // Box with a raised panel: 1 + 0.8 x 0.8 x 0.2.
    assert!(near(volume("poly_panel"), 1.0 + 0.8 * 0.8 * 0.2, 1.0e-5));
    // Two boxes joined by a 1 x 1 x 2 tunnel.
    assert!(near(volume("poly_tunnel"), 4.0, 1.0e-5));
    // Four vertical edges rounded: the unit block loses four corner areas times its height; each
    // 3-segment profile removes between the circle's and the chamfer's area.
    let rounded = volume("poly_bevel_block");
    let width: f64 = 0.15;
    let most = 4.0 * width * width / 2.0;
    let least = 4.0 * width * width * (1.0 - std::f64::consts::FRAC_PI_4);
    assert!(
        1.0 - rounded > least - 1.0e-6 && 1.0 - rounded < most,
        "removed {}",
        1.0 - rounded
    );
}

#[test]
fn uv_grid_survives_subdivision_and_evaluates_for_the_browser_viewport() {
    let result = evaluated("uv_subdivided_grid");
    assert_eq!(result.parts.len(), 1);
    assert!(result.parts[0].positions.len() > 9);
    assert!(result.parts[0].indices.len() >= 6 * 4 * 4 * 4);
}

#[test]
fn g8_gallery_repeat_and_creature_field_programs_evaluate_as_solids() {
    for (name, parts) in [("field_repeat_tube", 1), ("field_creature", 2)] {
        let result = evaluated(name);
        assert_eq!(result.parts.len(), parts, "{name}");
        assert!(
            result.parts.iter().all(|part| {
                part.volume.is_finite() && part.volume > 0.0 && !part.indices.is_empty()
            }),
            "{name}: invalid field output"
        );
    }
}

#[test]
fn g8_creature_preview_stays_within_triangle_budget() {
    let preview = evaluated_at("field_creature", Quality::Preview);
    let triangles: usize = preview
        .parts
        .iter()
        .map(|part| part.indices.len() / 3)
        .sum();
    assert!(triangles <= 200_000, "{triangles}");
    assert!(triangles > 0);

    let final_result = evaluated("field_creature");
    assert_eq!(preview.parts.len(), final_result.parts.len());
    let preview_volume: f64 = preview.parts.iter().map(|part| part.volume).sum();
    let final_volume: f64 = final_result.parts.iter().map(|part| part.volume).sum();
    assert!(near(preview_volume, final_volume, 0.12));
    for (preview_part, final_part) in preview.parts.iter().zip(&final_result.parts) {
        let (preview_min, preview_max) = part_bounds(preview_part);
        let (final_min, final_max) = part_bounds(final_part);
        for axis in 0..3 {
            assert!((preview_min[axis] - final_min[axis]).abs() <= 0.06);
            assert!((preview_max[axis] - final_max[axis]).abs() <= 0.06);
        }
    }
}

#[test]
fn g7_subdivided_bevelled_block_is_a_closed_smooth_solid() {
    let result = evaluated("poly_subdivided_block");
    assert_eq!(result.parts.len(), 1);
    assert!(result.parts[0].volume > 0.0);
    assert!(result.parts[0].indices.len() > 100);
}

#[test]
fn the_table_is_a_slab_on_four_mirrored_legs() {
    // Slab 1.2 x 0.04 x 0.7 plus four 0.05 x 0.05 legs of 0.70 m below it.
    let analytic = 1.2 * 0.04 * 0.7 + 4.0 * 0.05 * 0.05 * 0.70;
    assert!(
        near(volume("table"), analytic, 1.0e-4),
        "{} against {analytic}",
        volume("table")
    );
}

#[test]
fn the_gear_has_its_disc_teeth_and_bore() {
    // Disc (a 128-gon), sixteen teeth of which half sticks out, minus the 64-gon bore.
    let disc = 0.5 * 128.0 * 0.01 * (std::f64::consts::TAU / 128.0).sin() * 0.02;
    let teeth = 16.0 * 0.01 * 0.016 * 0.02;
    let bore = 0.5 * 64.0 * 0.0004 * (std::f64::consts::TAU / 64.0).sin() * 0.02;
    let analytic = disc + teeth - bore;
    assert!(
        near(volume("gear"), analytic, 0.02),
        "{} against {analytic}",
        volume("gear")
    );
}

#[test]
fn the_spiral_stair_has_twelve_treads_around_a_column() {
    // Column plus twelve treads, each sharing a half disc of the column with it.
    let column = 0.5 * 48.0 * 0.0064 * (std::f64::consts::TAU / 48.0).sin() * 2.2;
    let overlap = 12.0 * 0.5 * std::f64::consts::PI * 0.0064 * 0.05;
    let analytic = column + 12.0 * 0.4 * 0.05 * 0.2 - overlap;
    assert!(
        near(volume("spiral_stair"), analytic, 0.02),
        "{} against {analytic}",
        volume("spiral_stair")
    );
}

#[test]
fn the_twisted_tower_has_the_tapered_prism_volume() {
    let star_area = 5.0 * 0.1 * 0.045 * (std::f64::consts::PI / 5.0).sin();
    let analytic = star_area * 0.5 * (1.0 + 0.6 + 0.36) / 3.0;
    assert!(
        near(volume("twisted_tower"), analytic, 0.02),
        "{} against {analytic}",
        volume("twisted_tower")
    );
}

#[test]
fn the_offset_plate_matches_the_plate_plus_rim_volume() {
    // Area of a 140 x 80 mm rectangle grown by r with round corners: w*h + 2r(w+h) + pi r^2.
    let grown = |r: f64| 0.14 * 0.08 + 2.0 * r * (0.14 + 0.08) + std::f64::consts::PI * r * r;
    let hole = 0.0002 * 0.0002 + 4.0 * 0.0029 * 0.0002 + std::f64::consts::PI * 0.0029 * 0.0029;
    let base = (grown(0.01) - 4.0 * hole) * 0.005;
    let rim = (grown(0.006) - grown(0.003)) * 0.002;
    assert!(
        near(volume("offset_plate"), base + rim, 3.0e-3),
        "{} against {}",
        volume("offset_plate"),
        base + rim
    );
}
