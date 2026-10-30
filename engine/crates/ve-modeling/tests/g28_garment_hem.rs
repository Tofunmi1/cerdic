//! `body.garment` with `hem=true` cuts a clean edge: the waist hem of a shirt lies in one horizontal plane and a cuff in one plane across the arm, instead of
//! following the body's ragged skin regions. Written before the change; do not weaken.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup unwraps data that is known to be valid"
)]
use std::collections::BTreeMap;
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality, set_human_pack};

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/human/human.pack");
    set_human_pack(&std::fs::read(path).unwrap()).unwrap();
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| format!("{e:?}"))?
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g28_hem").unwrap(),
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

fn shirt(extra: &str) -> ve_modeling::Part {
    let source = format!(
        "body = body.human@1()\nroot_n = body.bone@1(body, bone=\"root\")\ns = body.garment@1(body, cover=\"{}\", offset=0.008, bones=\"root\"{extra})\noutput root = root_n\noutput shirt_part = s\n",
        "long_sleeves"
    );
    evaluate(&source)
        .unwrap()
        .parts
        .into_iter()
        .find(|p| p.name == "shirt_part")
        .unwrap()
}

/// Positions of the vertices on boundary edges (edges used by exactly one triangle), welded by position to a tenth of a millimetre.
fn boundary(part: &ve_modeling::Part) -> Vec<[f32; 3]> {
    let key = |i: u32| {
        let p = part.positions[i as usize];
        [
            (p[0] * 1e4).round() as i64,
            (p[1] * 1e4).round() as i64,
            (p[2] * 1e4).round() as i64,
        ]
    };
    let mut edges: BTreeMap<([i64; 3], [i64; 3]), u32> = BTreeMap::new();
    for t in part.indices.chunks(3) {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (ka, kb) = (key(a), key(b));
            *edges
                .entry(if ka < kb { (ka, kb) } else { (kb, ka) })
                .or_default() += 1;
        }
    }
    let mut out = Vec::new();
    for ((a, b), count) in edges {
        if count == 1 {
            for k in [a, b] {
                out.push([k[0] as f32 / 1e4, k[1] as f32 / 1e4, k[2] as f32 / 1e4]);
            }
        }
    }
    out
}

fn spread(values: impl Iterator<Item = f32>) -> f32 {
    let (lo, hi) = values.fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v), hi.max(v)));
    hi - lo
}

#[test]
fn a_plain_garment_has_a_ragged_edge_and_a_hemmed_one_does_not() {
    let plain = shirt("");
    let waist = |part: &ve_modeling::Part| {
        boundary(part)
            .into_iter()
            .filter(|p| p[1] < 1.15 && p[0].abs() < 0.3)
            .collect::<Vec<_>>()
    };
    let ragged = spread(waist(&plain).iter().map(|p| p[1]));
    assert!(
        ragged > 0.01,
        "the plain hem is ragged ({ragged} m); if this changed the fixture needs rethinking"
    );
    let neat = shirt(", hem=true");
    let hem = waist(&neat);
    assert!(
        hem.len() > 20,
        "a hem exists: {} boundary points",
        hem.len()
    );
    let flat = spread(hem.iter().map(|p| p[1]));
    assert!(
        flat < 0.004,
        "the waist hem lies in one horizontal plane: spread {flat} m"
    );
}

#[test]
fn a_cuff_is_cut_square_across_the_arm() {
    let neat = shirt(", hem=true");
    // the arms run along x in the rest pose: each cuff is a loop of boundary points far from the body
    for sign in [-1.0_f32, 1.0] {
        let cuff: Vec<[f32; 3]> = boundary(&neat)
            .into_iter()
            .filter(|p| p[0] * sign > 0.3)
            .collect();
        assert!(
            cuff.len() > 10,
            "side {sign}: a cuff exists ({} points)",
            cuff.len()
        );
        let flat = spread(cuff.iter().map(|p| p[0]));
        assert!(
            flat < 0.004,
            "side {sign}: the cuff lies in one plane across the arm: spread {flat} m"
        );
    }
}

#[test]
fn hemming_keeps_the_garment_whole_and_skinned() {
    let plain = shirt("");
    let neat = shirt(", hem=true");
    let (a, b) = (plain.indices.len() as f32, neat.indices.len() as f32);
    assert!(
        (b / a - 1.0).abs() < 0.15,
        "about the same amount of cloth: {a} against {b} indices"
    );
    assert!(neat.skin.is_some() && plain.skin.is_some(), "still skinned");
    assert_eq!(
        neat.skin.as_ref().unwrap().weights.len(),
        neat.positions.len(),
        "one weight set per vertex"
    );
    assert_eq!(
        neat.skin.as_ref().unwrap().joints.len(),
        neat.positions.len(),
        "and one joint set"
    );
    assert!(
        neat.indices
            .iter()
            .all(|&i| (i as usize) < neat.positions.len()),
        "valid indices"
    );
    assert!(
        neat.positions
            .iter()
            .all(|p| p.iter().all(|c| c.is_finite())),
        "finite positions"
    );
}

#[test]
fn hem_is_optional_and_off_by_default() {
    let plain = shirt("");
    let off = shirt(", hem=false");
    assert_eq!(plain.positions, off.positions);
}
