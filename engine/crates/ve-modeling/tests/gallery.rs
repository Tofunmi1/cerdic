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

fn near(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance * b
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
