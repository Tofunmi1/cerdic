//! `city.ground` and `city.buildings`: the district as parts. Every part and style is its own node on the same streets, so together they are the whole district exactly once.
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
        transaction: TransactionId::parse("t_g33_city").unwrap(),
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

fn triangles(source: &str, name: &str) -> usize {
    let result = evaluate(source).unwrap();
    result
        .parts
        .iter()
        .find(|p| p.name == name)
        .map_or(0, |p| p.indices.len() / 3)
}

#[test]
fn the_styles_of_one_part_partition_the_district() {
    let whole = triangles(
        &format!(
            "b = city.buildings@1({LAYOUT}, part=\"walls\", styles=1, style=0)\noutput b = b\n"
        ),
        "b",
    );
    let split: usize = (0..4).map(|s| triangles(&format!("b = city.buildings@1({LAYOUT}, part=\"walls\", styles=4, style={s})\noutput b = b\n"), "b")).sum();
    assert!(whole > 500, "a district has hundreds of walls: {whole}");
    assert_eq!(
        whole, split,
        "four styles together are the one-style district"
    );
}

#[test]
fn walls_shops_and_roofs_are_different_meshes_with_uvs() {
    for part in ["shop", "walls", "roof"] {
        let result = evaluate(&format!(
            "b = city.buildings@1({LAYOUT}, part=\"{part}\")\noutput b = b\n"
        ))
        .unwrap();
        let p = &result.parts[0];
        assert!(!p.indices.is_empty(), "{part}");
        assert_eq!(p.uvs.len(), p.indices.len(), "{part}: a UV per corner");
    }
    assert!(
        triangles(
            &format!("b = city.buildings@1({LAYOUT}, part=\"roof\", clutter=1)\noutput b = b\n"),
            "b"
        ) > triangles(
            &format!("b = city.buildings@1({LAYOUT}, part=\"roof\", clutter=0)\noutput b = b\n"),
            "b"
        )
    );
}

#[test]
fn the_ground_has_a_road_and_pavements_and_buildings_stand_on_it() {
    let road = evaluate(&format!(
        "g = city.ground@1({LAYOUT}, part=\"road\")\noutput g = g\n"
    ))
    .unwrap();
    assert_eq!(
        road.parts[0].indices.len() / 3,
        4,
        "the road is one quad under the district"
    );
    let pavement = triangles(
        &format!("g = city.ground@1({LAYOUT}, part=\"pavement\")\noutput g = g\n"),
        "g",
    );
    assert!(pavement > 100, "{pavement}");
    let walls = evaluate(&format!(
        "b = city.buildings@1({LAYOUT}, part=\"walls\", height_scale=2)\noutput b = b\n"
    ))
    .unwrap();
    let top = walls.parts[0]
        .positions
        .iter()
        .map(|p| p[1])
        .fold(0.0_f32, f32::max);
    assert!(top > 100.0, "downtown towers: {top} m");
}

#[test]
fn the_default_district_fits_a_headset_budget() {
    let program = "g = city.ground@1(part=\"pavement\")\nw = city.buildings@1(part=\"walls\")\ns = city.buildings@1(part=\"shop\")\nr = city.buildings@1(part=\"roof\")\noutput g = g\noutput w = w\noutput s = s\noutput r = r\n";
    let result = evaluate(program).unwrap();
    let total: usize = result.parts.iter().map(|p| p.indices.len() / 3).sum();
    assert!(total < 150_000, "{total} triangles");
}

#[test]
fn bad_values_are_refused() {
    for (source, word) in [
        (
            format!("b = city.buildings@1({LAYOUT}, styles=2, style=2)\noutput b = b\n"),
            "style",
        ),
        (
            "b = city.buildings@1(block=5)\noutput b = b\n".to_owned(),
            "block",
        ),
        (
            "b = city.buildings@1(lot_min=30, lot_max=20)\noutput b = b\n".to_owned(),
            "lot_max",
        ),
    ] {
        let error = evaluate(&source)
            .err()
            .unwrap_or_else(|| panic!("{source} should be refused"));
        assert!(error.contains(word), "{word}: {error}");
    }
}
