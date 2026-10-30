//! `terrain.flatten` levels a pad in the ground (for a building, a camp, a road end) with a soft shoulder, and `terrain.place` stands
//! copies of any shape on the ground at listed x,z positions. Written before the body; do not weaken.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup unwraps data that is known to be valid"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| format!("{e:?}"))?
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g21_place").unwrap(),
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

/// A 64 m tile at 2 m: grid points at every even x and z from 0 to 64.
const TILE: &str = "t = terrain.tile@1(size=[64, 64], cell=2, seed=3, height=40, scale=60)\n";

fn positions(source: &str) -> Vec<[f32; 3]> {
    evaluate(source).unwrap().parts[0].positions.clone()
}

fn at(points: &[[f32; 3]], x: f32, z: f32) -> f32 {
    points
        .iter()
        .find(|p| (p[0] - x).abs() < 1e-4 && (p[2] - z).abs() < 1e-4)
        .map(|p| p[1])
        .unwrap()
}

fn distance(p: &[f32; 3], x: f32, z: f32) -> f32 {
    ((p[0] - x).powi(2) + (p[2] - z).powi(2)).sqrt()
}

#[test]
fn flatten_levels_a_pad_to_a_height_and_leaves_far_ground_alone() {
    let plain = positions(&format!("{TILE}output t = t\n"));
    let flat = positions(&format!(
        "{TILE}f = terrain.flatten@1(t, at=[32, 32], radius=6, blend=8, to=\"height\", height=15)\noutput f = f\n"
    ));
    assert_eq!(plain.len(), flat.len());
    let mut inside = 0;
    let mut shoulder_changed = 0;
    for (p, f) in plain.iter().zip(&flat) {
        let d = distance(p, 32.0, 32.0);
        if d <= 6.0 {
            inside += 1;
            assert!(
                (f[1] - 15.0).abs() < 1e-3,
                "the pad is level at 15 m: {f:?}"
            );
        } else if d >= 14.0 + 1e-3 {
            assert_eq!(
                p[1], f[1],
                "ground beyond the shoulder is untouched at {p:?}"
            );
        } else if (p[1] - f[1]).abs() > 1e-3 {
            shoulder_changed += 1;
        }
    }
    assert!(inside > 20, "the pad covers some grid points: {inside}");
    assert!(
        shoulder_changed > 10,
        "the shoulder blends the ground toward the pad: {shoulder_changed}"
    );
}

#[test]
fn flatten_can_level_to_the_ground_at_its_centre() {
    let plain = positions(&format!("{TILE}output t = t\n"));
    let flat = positions(&format!(
        "{TILE}f = terrain.flatten@1(t, at=[32, 32], radius=5, blend=4)\noutput f = f\n"
    ));
    let centre = at(&plain, 32.0, 32.0);
    for p in flat.iter().filter(|p| distance(p, 32.0, 32.0) <= 5.0) {
        assert!(
            (p[1] - centre).abs() < 1e-3,
            "the pad keeps the centre's height {centre}: {p:?}"
        );
    }
}

#[test]
fn place_stands_copies_on_the_ground_at_listed_positions() {
    let plain = positions(&format!("{TILE}output t = t\n"));
    let result = evaluate(&format!("{TILE}b = solid.box@1(size=[1, 1, 1])\np = terrain.place@1(b, t, at=[[10, 10], [40, 22]])\noutput p = p\n")).unwrap();
    let placed = result.parts[0].transforms.as_ref().unwrap();
    assert_eq!(placed.len(), 2);
    assert_eq!([placed[0][12], placed[0][14]], [10.0, 10.0]);
    assert!(
        (placed[0][13] - at(&plain, 10.0, 10.0)).abs() < 1e-3,
        "the first copy stands on the ground"
    );
    assert!(
        (placed[1][13] - at(&plain, 40.0, 22.0)).abs() < 1e-3,
        "the second copy stands on the ground"
    );
    assert_eq!(
        result.parts[0].indices.len() / 3,
        12,
        "the shape is stored once"
    );
}

#[test]
fn a_flattened_pad_is_where_a_placed_building_stands() {
    let result = evaluate(&format!(
        "{TILE}f = terrain.flatten@1(t, at=[32, 32], radius=6, blend=8, to=\"height\", height=25)\nb = solid.box@1(size=[1, 1, 1])\np = terrain.place@1(b, f, at=[[32, 32]], sink=0.5)\noutput p = p\n"
    ))
    .unwrap();
    let placed = result.parts[0].transforms.as_ref().unwrap();
    assert!(
        (placed[0][13] - 24.5).abs() < 1e-3,
        "pad height 25 minus a 0.5 m sink: {}",
        placed[0][13]
    );
}

#[test]
fn place_turns_and_scales_each_copy_and_can_lean_with_the_slope() {
    let result = evaluate(&format!(
        "{TILE}b = solid.box@1(size=[1, 1, 1])\np = terrain.place@1(b, t, at=[[10, 10], [40, 22]], yaw_scale=[[90, 1], [0, 2]])\noutput p = p\n"
    ))
    .unwrap();
    let placed = result.parts[0].transforms.as_ref().unwrap();
    assert!(
        placed[0][0].abs() < 1e-5 && (placed[0][2] + 1.0).abs() < 1e-5,
        "a quarter turn sends +X to -Z"
    );
    assert!((placed[1][0] - 2.0).abs() < 1e-5, "scale 2 doubles +X");
    let leaning = evaluate(&format!("{TILE}b = solid.box@1(size=[1, 1, 1])\np = terrain.place@1(b, t, at=[[10, 10]], align=1)\noutput p = p\n")).unwrap();
    let m = &leaning.parts[0].transforms.as_ref().unwrap()[0];
    assert!(
        (m[5] * m[5] + m[4] * m[4] + m[6] * m[6] - 1.0).abs() < 1e-4,
        "the up axis is still unit length"
    );
    assert!(
        m[5] < 1.0 - 1e-4,
        "on sloping ground the up axis leans: {}",
        m[5]
    );
}

#[test]
fn a_position_off_the_tile_is_refused_with_the_tile_bounds() {
    let error = evaluate(&format!("{TILE}b = solid.box@1(size=[1, 1, 1])\np = terrain.place@1(b, t, at=[[10, 10], [500, 22]])\noutput p = p\n")).unwrap_err();
    assert!(
        error.contains("500") && error.contains("tile"),
        "the message names the position and the tile: {error}"
    );
}

#[test]
fn a_list_of_turns_must_match_the_list_of_positions() {
    let error = evaluate(&format!("{TILE}b = solid.box@1(size=[1, 1, 1])\np = terrain.place@1(b, t, at=[[10, 10], [20, 20]], yaw_scale=[[90, 1]])\noutput p = p\n")).unwrap_err();
    assert!(
        error.contains("yaw_scale") && error.contains('2'),
        "the message says what to match: {error}"
    );
}
