//! The New York detail of `city.buildings` and the long blocks of the city nodes, reached through the language: `detail` adds the parts `metal` (fire escapes) and
//! `awning`, `block_depth` makes long thin blocks, and an empty detail part says what to raise. With detail 0 and no block_depth a district is what it was.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup unwraps data that is known to be valid"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

const LAYOUT: &str =
    "size=[200, 200], block=40, street_width=9, lot_min=6, lot_max=14, irregularity=0.4, seed=5";

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| format!("{e:?}"))?
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g33b_city").unwrap(),
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

fn triangles(layout: &str, part: &str, extra: &str) -> Result<usize, String> {
    let result = evaluate(&format!(
        "b = city.buildings@1({layout}, part=\"{part}\"{extra})\noutput b = b\n"
    ))?;
    Ok(result
        .parts
        .iter()
        .find(|p| p.name == "b")
        .map_or(0, |p| p.indices.len() / 3))
}

#[test]
fn detail_zero_is_the_district_it_always_was() {
    let plain = triangles(LAYOUT, "walls", "").unwrap();
    let explicit = triangles(LAYOUT, "walls", ", detail=0").unwrap();
    assert_eq!(plain, explicit);
}

#[test]
fn detail_adds_fire_escapes_and_awnings_as_their_own_parts() {
    let metal = triangles(LAYOUT, "metal", ", detail=1").unwrap();
    let awning = triangles(LAYOUT, "awning", ", detail=1").unwrap();
    assert!(metal > 500, "a district has fire escapes: {metal}");
    assert!(awning > 200, "and awnings: {awning}");
}

#[test]
fn detail_makes_the_walls_and_roofs_richer() {
    assert!(
        triangles(LAYOUT, "walls", ", detail=1").unwrap()
            > triangles(LAYOUT, "walls", ", detail=0").unwrap()
    );
    assert!(
        triangles(LAYOUT, "roof", ", detail=1").unwrap()
            > triangles(LAYOUT, "roof", ", detail=0").unwrap()
    );
}

#[test]
fn an_empty_detail_part_says_what_to_raise() {
    let err = triangles(LAYOUT, "metal", ", detail=0").unwrap_err();
    assert!(
        err.contains("detail"),
        "the message names the parameter to raise: {err}"
    );
}

#[test]
fn block_depth_gives_a_district_of_long_thin_blocks() {
    let square = triangles(LAYOUT, "walls", "").unwrap();
    let long = triangles("size=[200, 200], block=40, block_depth=120, street_width=9, lot_min=6, lot_max=14, irregularity=0.4, seed=5", "walls", "").unwrap();
    assert!(
        long > 100 && long != square,
        "a different grid of streets: {long} against {square}"
    );
}

#[test]
fn ground_and_buildings_share_the_same_long_blocks() {
    let layout = "size=[200, 200], block=40, block_depth=120, street_width=9, lot_min=6, lot_max=14, irregularity=0.4, seed=5";
    let result = evaluate(&format!(
        "g = city.ground@1({layout}, part=\"pavement\")\noutput g = g\n"
    ))
    .unwrap();
    assert!(
        result
            .parts
            .iter()
            .any(|p| p.name == "g" && !p.indices.is_empty())
    );
}

#[test]
fn a_block_depth_between_zero_and_fifteen_is_refused() {
    let err = evaluate(&format!(
        "g = city.ground@1(size=[200, 200], block_depth=5, part=\"road\")\noutput g = g\n"
    ))
    .unwrap_err();
    assert!(err.contains("block_depth"), "{err}");
}
