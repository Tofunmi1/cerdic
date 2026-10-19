//! `bake.maps`: a rounded box's detail baked onto a plain box's UV layout, from the text a model writes to the maps on the part.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "the helpers unwrap fixtures that are known to be valid and report failures by panicking"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

const SCENE: &str = "box = solid.box@1(size=[1, 1, 1])\n\
    tris = poly.from_solid@1(box)\n\
    flat = poly.merge_coplanar@1(tris)\n\
    soft = poly.round_edges@1(flat, width=0.12, segments=4)\n\
    high = poly.to_solid@1(soft)\n\
    uv = poly.unwrap_uv@1(flat)\n\
    low = poly.to_solid@1(uv)\n";

fn evaluate(text: &str) -> Result<Evaluation, (String, Option<String>)> {
    let batch = parse_text(&builtin_registry(), text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g11_bake_maps").unwrap(),
        label: None,
        commands: batch.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.suggestion.clone()))
}

fn run(text: &str) -> Evaluation {
    evaluate(text).unwrap_or_else(|(code, _)| panic!("{code}"))
}

fn baked(settings: &str) -> Evaluation {
    run(&format!(
        "{SCENE}baked = bake.maps@1(low, high{settings})\noutput baked"
    ))
}

#[test]
fn both_maps_land_on_the_low_part_without_changing_its_shape() {
    let result = baked(", size=128, rays=16");
    let part = &result.parts[0];
    let maps = part.baked.as_ref().expect("the part carries the bake");
    assert_eq!(maps.size, 128);
    assert_eq!(maps.normal.as_ref().unwrap().len(), 128 * 128 * 4);
    assert_eq!(maps.ao.as_ref().unwrap().len(), 128 * 128 * 4);
    // The geometry is the low part's, untouched.
    let plain = run(&format!("{SCENE}output low"));
    assert_eq!(part.positions, plain.parts[0].positions);
    assert_eq!(part.indices, plain.parts[0].indices);
    assert_eq!(part.uvs, plain.parts[0].uvs);
    assert!((part.volume - plain.parts[0].volume).abs() < 1.0e-9);
    assert!(plain.parts[0].baked.is_none());
}

#[test]
fn the_normal_map_is_flat_on_the_faces_and_tilted_along_the_bevels() {
    let result = baked(", size=128, maps=\"normal\"");
    let normal = result.parts[0]
        .baked
        .as_ref()
        .unwrap()
        .normal
        .as_ref()
        .unwrap();
    let (mut flat, mut tilted) = (0, 0);
    for pixel in normal.chunks_exact(4).filter(|p| p[3] == 255) {
        let off = |c: u8, centre: i32| (i32::from(c) - centre).abs();
        if off(pixel[0], 128) <= 2 && off(pixel[1], 128) <= 2 && off(pixel[2], 255) <= 2 {
            flat += 1;
        } else if off(pixel[0], 128) > 20 || off(pixel[1], 128) > 20 {
            tilted += 1;
        }
    }
    assert!(flat > 3000, "only {flat} flat texels");
    assert!(
        tilted > 600,
        "only {tilted} tilted texels, the bevel bands should show"
    );
}

#[test]
fn which_maps_are_baked_follows_the_maps_setting() {
    let only_normal = baked(", size=64, maps=\"normal\"");
    let maps = only_normal.parts[0].baked.as_ref().unwrap();
    assert!(maps.normal.is_some() && maps.ao.is_none());
    let only_ao = baked(", size=64, rays=8, maps=\"ao\"");
    let maps = only_ao.parts[0].baked.as_ref().unwrap();
    assert!(maps.normal.is_none() && maps.ao.is_some());
}

#[test]
fn a_convex_box_is_fully_open_to_the_sky() {
    let result = baked(", size=64, rays=32, maps=\"ao\"");
    let ao = result.parts[0].baked.as_ref().unwrap().ao.as_ref().unwrap();
    let darkest = ao
        .chunks_exact(4)
        .filter(|p| p[3] == 255)
        .map(|p| p[0])
        .min()
        .unwrap();
    assert!(darkest >= 245, "darkest covered texel is {darkest}");
}

#[test]
fn a_low_part_without_uvs_is_refused_with_the_fix() {
    let (code, suggestion) = evaluate(&format!(
        "{SCENE}raw = solid.box@1(size=[1, 1, 1])\nbaked = bake.maps@1(raw, high)\noutput baked"
    ))
    .unwrap_err();
    assert_eq!(code, "bake.no_uvs");
    assert!(suggestion.unwrap().contains("poly.unwrap_uv"));
}

#[test]
fn a_bake_too_big_to_run_is_refused_with_a_budget_fix() {
    let (code, suggestion) = evaluate(&format!(
        "{SCENE}baked = bake.maps@1(low, high, size=2048, rays=64)\noutput baked"
    ))
    .unwrap_err();
    assert_eq!(code, "budget.bake");
    let hint = suggestion.unwrap();
    assert!(hint.contains("size") && hint.contains("rays"), "{hint}");
}

#[test]
fn a_bake_is_repeatable() {
    let first = baked(", size=64, rays=8");
    let second = baked(", size=64, rays=8");
    assert_eq!(first.parts[0].baked, second.parts[0].baked);
}

#[test]
fn the_size_can_come_from_a_value_node() {
    let result = run(&format!(
        "{SCENE}n = value.number@1(value=64)\nbaked = bake.maps@1(low, high, size=n, maps=\"normal\")\noutput baked"
    ));
    assert_eq!(result.parts[0].baked.as_ref().unwrap().size, 64);
}

#[test]
fn the_maps_stay_with_the_part_through_a_move() {
    let result = run(&format!(
        "{SCENE}baked = bake.maps@1(low, high, size=64, maps=\"normal\")\nmoved = xform.move@1(baked, by=[2, 0, 0])\noutput moved"
    ));
    let still = baked(", size=64, maps=\"normal\"");
    assert_eq!(result.parts[0].baked, still.parts[0].baked);
    assert!(result.parts[0].baked.is_some());
}
