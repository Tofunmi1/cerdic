//! The human body model: a pack of CC0 data (ADR 0055) evaluated to a body from named parameters. The golden numbers come from the reference model
//! (NAVER Anny, run by `tools/human-pack/convert.py` at the same settings), converted to our axes (x right, y up, z forward, metres).
#![expect(
    clippy::unwrap_used,
    clippy::float_cmp,
    reason = "fixture setup unwraps data that is known to be valid and reports actionable failures"
)]
use std::collections::BTreeMap;
use std::sync::OnceLock;

use ve_human::{Human, Pack, Params};

fn human() -> &'static Human {
    static HUMAN: OnceLock<Human> = OnceLock::new();
    HUMAN.get_or_init(|| {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/human/human.pack");
        let bytes = std::fs::read(path).unwrap();
        Human::new(Pack::from_zstd(&bytes).unwrap())
    })
}

fn params(pairs: &[(&str, f32)]) -> Params {
    let mut p = Params::default();
    for (name, value) in pairs {
        p.phenotype.insert((*name).to_owned(), *value);
    }
    p
}

fn height(positions: &[[f32; 3]]) -> f32 {
    let (lo, hi) = positions.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| {
        (lo.min(p[1]), hi.max(p[1]))
    });
    hi - lo
}

#[test]
fn the_pack_loads_with_the_expected_sizes_and_labels() {
    let h = human();
    assert_eq!(h.vertex_count(), 13_718);
    assert_eq!(h.bone_names().len(), 104);
    assert_eq!(h.bone_names()[0], "root");
    for label in [
        "gender",
        "age",
        "muscle",
        "weight",
        "height",
        "proportions",
        "african",
        "asian",
        "caucasian",
    ] {
        assert!(
            h.phenotype_labels().iter().any(|l| l == label),
            "phenotype {label}"
        );
    }
    assert_eq!(h.local_change_labels().len(), 256);
    assert_eq!(h.face_action_labels().len(), 52);
    assert!(h.face_action_labels().iter().any(|l| l == "jawOpen"));
}

#[test]
fn a_corrupt_pack_is_refused_not_a_panic() {
    assert!(Pack::from_zstd(b"not a pack").is_err());
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/human/human.pack");
    let mut bytes = std::fs::read(path).unwrap();
    bytes.truncate(bytes.len() / 2);
    assert!(Pack::from_zstd(&bytes).is_err());
}

#[test]
fn it_matches_the_reference_model_at_every_golden_setting() {
    #[derive(serde::Deserialize)]
    struct Golden {
        phenotype: BTreeMap<String, f32>,
        local: BTreeMap<String, f32>,
        face: BTreeMap<String, f32>,
        sample: Vec<usize>,
        positions: Vec<[f32; 3]>,
    }
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/human/human.golden.json"
    );
    let golden: Vec<Golden> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(golden.len(), 5);
    for case in golden {
        let body = human()
            .evaluate(&Params {
                phenotype: case.phenotype.clone(),
                local: case.local.clone(),
                face: case.face.clone(),
            })
            .unwrap();
        let worst = case
            .sample
            .iter()
            .zip(&case.positions)
            .map(|(&i, want)| {
                let got = body.positions[i];
                (0..3)
                    .map(|a| (got[a] - want[a]).abs())
                    .fold(0.0_f32, f32::max)
            })
            .fold(0.0_f32, f32::max);
        // The pack keeps a low-rank basis (error under 2.5 mm by construction) and 16-bit deltas: 4 mm is the contract.
        assert!(
            worst < 0.004,
            "{:?}: worst sampled error {worst} m",
            case.phenotype
        );
    }
}

#[test]
fn the_default_body_is_an_adult_human_height_and_standing_on_a_plausible_origin() {
    let body = human().evaluate(&Params::default()).unwrap();
    let h = height(&body.positions);
    assert!((1.5..1.85).contains(&h), "default height {h}");
    // Facing +z: the nose is the most forward point of the head, in front of the chest centre.
    let head = body
        .positions
        .iter()
        .filter(|p| p[1] > 0.5)
        .map(|p| p[2])
        .fold(f32::MIN, f32::max);
    assert!(head > 0.05, "the face points toward +z: {head}");
}

#[test]
fn height_age_and_gender_move_the_body_the_right_way() {
    let h = |pairs: &[(&str, f32)]| height(&human().evaluate(&params(pairs)).unwrap().positions);
    assert!(
        h(&[("height", 1.0)]) > h(&[("height", 0.0)]) + 0.05,
        "taller when asked"
    );
    assert!(
        h(&[("age", 0.1)]) < h(&[("age", 0.6)]) - 0.4,
        "a child is much shorter than an adult"
    );
    assert!(
        h(&[("gender", 0.0)]) > h(&[("gender", 1.0)]) + 0.03,
        "gender 0 is the taller (male) end, as in the reference model"
    );
}

#[test]
fn evaluating_twice_gives_identical_bytes() {
    let p = params(&[("age", 0.7), ("weight", 0.3), ("muscle", 0.9)]);
    assert_eq!(
        human().evaluate(&p).unwrap().positions,
        human().evaluate(&p).unwrap().positions
    );
}

#[test]
fn a_face_action_moves_the_face_and_nothing_below_the_neck() {
    let rest = human().evaluate(&Params::default()).unwrap();
    let mut open = Params::default();
    open.face.insert("jawOpen".to_owned(), 1.0);
    let moved = human().evaluate(&open).unwrap();
    let travel = |i: usize| {
        (0..3)
            .map(|a| (moved.positions[i][a] - rest.positions[i][a]).powi(2))
            .sum::<f32>()
            .sqrt()
    };
    let biggest = (0..rest.positions.len())
        .map(travel)
        .fold(0.0_f32, f32::max);
    assert!(
        biggest > 0.01,
        "the jaw drops by more than a centimetre: {biggest}"
    );
    for i in 0..rest.positions.len() {
        if rest.positions[i][1] < rest.positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max) - 0.4
        {
            assert!(travel(i) < 1e-6, "vertex {i} below the neck did not move");
        }
    }
}

#[test]
fn a_local_change_is_confined_to_its_region() {
    let name = human()
        .local_change_labels()
        .iter()
        .find(|l| l.contains("upperarm-fat"))
        .unwrap()
        .clone();
    let rest = human().evaluate(&Params::default()).unwrap();
    let mut changed = Params::default();
    changed.local.insert(name, 1.0);
    let moved = human().evaluate(&changed).unwrap();
    let count = (0..rest.positions.len())
        .filter(|&i| (0..3).any(|a| (moved.positions[i][a] - rest.positions[i][a]).abs() > 1e-5))
        .count();
    assert!(
        count > 20 && count < 3000,
        "the arm changes and little else: {count} vertices moved"
    );
}

#[test]
fn unknown_and_out_of_range_parameters_are_refused_naming_the_fix() {
    let mut bad = Params::default();
    bad.phenotype.insert("heigth".to_owned(), 0.5);
    let error = format!("{:?}", human().evaluate(&bad).unwrap_err());
    assert!(error.contains("heigth"), "{error}");
    assert!(
        error.contains("height"),
        "the message lists the valid names: {error}"
    );
    let mut range = Params::default();
    range.phenotype.insert("age".to_owned(), 1.5);
    assert!(human().evaluate(&range).is_err());
    let mut face = Params::default();
    face.face.insert("jawOpen".to_owned(), -0.1);
    assert!(human().evaluate(&face).is_err());
    let mut local = Params::default();
    local
        .local
        .insert(human().local_change_labels()[0].clone(), 1.5);
    assert!(human().evaluate(&local).is_err());
}

#[test]
fn the_skin_has_weights_that_sum_to_one_and_valid_bones() {
    let skin = human().skin();
    assert_eq!(skin.len(), human().vertex_count());
    for (i, influences) in skin.iter().enumerate() {
        let total: f32 = influences.iter().map(|&(_, w)| w).sum();
        assert!(
            (total - 1.0).abs() < 2e-3,
            "vertex {i}: weights sum to {total}"
        );
        assert!(
            influences.iter().all(|&(b, _)| (b as usize) < 104),
            "vertex {i}"
        );
    }
}

#[test]
fn the_skeleton_follows_the_shape_and_has_one_root() {
    let tall = human().evaluate(&params(&[("height", 1.0)])).unwrap();
    let short = human().evaluate(&params(&[("height", 0.0)])).unwrap();
    let parents = human().bone_parents();
    assert_eq!(parents.iter().filter(|&&p| p < 0).count(), 1);
    assert_eq!(parents[0], -1);
    let head = human()
        .bone_names()
        .iter()
        .position(|n| n == "head")
        .unwrap();
    assert!(tall.bone_heads[head][1] > short.bone_heads[head][1] + 0.03);
    assert_eq!(tall.bone_heads.len(), 104);
}

#[test]
fn the_render_mesh_is_a_valid_triangle_list_with_uvs() {
    let h = human();
    let mesh = h.render_mesh();
    assert!(mesh.triangles.len() > 20_000);
    let r = mesh.source.len();
    assert_eq!(mesh.uv.len(), r);
    assert!(mesh.source.iter().all(|&s| (s as usize) < h.vertex_count()));
    assert!(
        mesh.triangles
            .iter()
            .all(|t| t.iter().all(|&i| (i as usize) < r))
    );
    assert!(
        mesh.uv
            .iter()
            .all(|uv| (-0.001..=1.001).contains(&uv[0]) && (-0.001..=1.001).contains(&uv[1]))
    );
    // The evaluated body gives one position per render vertex too.
    let body = h.evaluate(&Params::default()).unwrap();
    assert_eq!(body.render_positions.len(), r);
    assert_eq!(
        body.render_positions[0],
        body.positions[mesh.source[0] as usize]
    );
}
