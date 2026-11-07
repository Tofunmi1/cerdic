//! Building shape of `city.buildings` through the language: `mansard` (the share of mid-rise buildings with a mansard roof and dormers) and `courtyards` (the share
//! of big lots split into an L or a U). Both default to 0, which is the district it always was. Written before the code; do not weaken.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup unwraps data that is known to be valid"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

const LAYOUT: &str = "size=[240, 240], block=90, street_width=12, lot_min=18, lot_max=34, irregularity=0, seed=5, height_scale=1.5";

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| format!("{e:?}"))?
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g33c_city").unwrap(),
        label: None,
        commands,
    });
    if !matches!(applied, Outcome::Committed { .. }) {
        return Err(format!("{applied:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| format!("{} {}", e.code, e.message))
}

fn triangles(part: &str, extra: &str) -> Result<usize, String> {
    let result = evaluate(&format!(
        "b = city.buildings@1({LAYOUT}, part=\"{part}\"{extra})\noutput b = b\n"
    ))?;
    Ok(result
        .parts
        .iter()
        .find(|p| p.name == "b")
        .map_or(0, |p| p.indices.len() / 3))
}

#[test]
fn shape_zero_is_the_district_it_always_was() {
    for part in ["walls", "roof"] {
        assert_eq!(
            triangles(part, "").unwrap(),
            triangles(part, ", mansard=0, courtyards=0").unwrap()
        );
    }
}

#[test]
fn mansard_roofs_with_dormers_add_roof_and_wall_triangles() {
    let flat_roof = triangles("roof", "").unwrap();
    let flat_walls = triangles("walls", "").unwrap();
    assert!(
        triangles("roof", ", mansard=1").unwrap() > flat_roof,
        "slopes and dormer caps"
    );
    assert!(
        triangles("walls", ", mansard=1").unwrap() > flat_walls,
        "dormer fronts"
    );
}

#[test]
fn courtyards_change_the_footprints() {
    let whole = triangles("walls", "").unwrap();
    let split = triangles("walls", ", courtyards=1").unwrap();
    assert!(
        split > 0 && split != whole,
        "wings are different buildings: {split} against {whole}"
    );
}

fn top(extra: &str) -> f32 {
    let result = evaluate(&format!(
        "b = city.buildings@1({LAYOUT}, part=\"roof\"{extra})\noutput b = b\n"
    ))
    .unwrap();
    result
        .parts
        .iter()
        .flat_map(|p| p.positions.iter())
        .map(|p| p[1])
        .fold(0.0, f32::max)
}

#[test]
fn height_max_caps_the_skyline_and_height_min_lifts_the_low_buildings() {
    let free = top(", clutter=0");
    assert!(
        free > 40.0,
        "the default district has tall buildings downtown: {free}"
    );
    assert!(
        top(", clutter=0, height_max=22") <= 22.01,
        "no building above the cap"
    );
    let result = evaluate(&format!("b = city.buildings@1({LAYOUT}, part=\"roof\", clutter=0, height_min=30, height_max=30)\noutput b = b\n")).unwrap();
    let lowest = result
        .parts
        .iter()
        .flat_map(|p| p.positions.iter())
        .map(|p| p[1])
        .filter(|y| *y > 5.0)
        .fold(f32::MAX, f32::min);
    assert!(lowest >= 29.9, "every roof is at the minimum: {lowest}");
}

#[test]
fn the_height_range_must_make_sense() {
    assert!(triangles("walls", ", height_min=40, height_max=20").is_err());
}

#[test]
fn the_shares_are_checked() {
    assert!(triangles("walls", ", mansard=1.5").is_err());
    assert!(triangles("walls", ", courtyards=-0.1").is_err());
}
