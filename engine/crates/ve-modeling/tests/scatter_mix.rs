//! `mix` and `pick`: several scatter.place nodes share one planting, each taking its own weighted slice, so variants never overlap.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use std::collections::BTreeSet;

use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let commands = parse_text(&builtin_registry(), source).map_err(|e| format!("{e:?}"))?.commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_mix").unwrap(),
        label: None,
        commands,
    });
    if !matches!(applied, Outcome::Committed { .. }) {
        return Err(format!("{applied:?}"));
    }
    Evaluator::default().evaluate(session.document(), Quality::Final).map_err(|e| format!("{e:?}"))
}

fn plantings(mixes: &[(&str, u32)]) -> Vec<Vec<[i32; 3]>> {
    let mut text = String::from("t = terrain.tile@1(size=[96, 96], cell=2, seed=4, height=40, scale=160)\nb = solid.box@1(size=[0.5, 1, 0.5])\n");
    for (index, (mix, pick)) in mixes.iter().enumerate() {
        text.push_str(&format!("f{index} = scatter.place@1(b, t, radius=3, seed=7, mix=\"{mix}\", pick={pick})\noutput f{index} = f{index}\n"));
    }
    let parts = evaluate(&text).unwrap().parts;
    (0..mixes.len())
        .map(|index| {
            let name = format!("f{index}");
            parts
                .iter()
                .find(|p| p.name == name)
                .and_then(|p| p.transforms.as_ref())
                .map(|list| list.iter().map(|m| [(m[12] * 100.0) as i32, (m[13] * 100.0) as i32, (m[14] * 100.0) as i32]).collect())
                .unwrap_or_default()
        })
        .collect()
}

#[test]
fn the_shares_of_a_mix_split_one_planting_without_overlap_or_loss() {
    let whole = &plantings(&[("1", 0)])[0];
    let parts = plantings(&[("5, 3, 2", 0), ("5, 3, 2", 1), ("5, 3, 2", 2)]);
    let total: usize = parts.iter().map(Vec::len).sum();
    assert_eq!(total, whole.len(), "every plant goes to exactly one variant");
    let everything: BTreeSet<[i32; 3]> = parts.iter().flatten().copied().collect();
    assert_eq!(everything.len(), total, "no plant is in two variants");
    let expected: BTreeSet<[i32; 3]> = whole.iter().copied().collect();
    assert_eq!(everything, expected, "the variants together are the whole planting");
    // the shares follow the weights, within the noise of a few hundred plants
    let fractions: Vec<f64> = parts.iter().map(|p| p.len() as f64 / total as f64).collect();
    for (got, want) in fractions.iter().zip([0.5, 0.3, 0.2]) {
        assert!((got - want).abs() < 0.12, "shares {fractions:?} against 0.5, 0.3, 0.2");
    }
}

#[test]
fn a_variant_is_the_same_whichever_neighbours_exist() {
    let alone = &plantings(&[("1, 1", 1)])[0];
    let with_friend = &plantings(&[("1, 1", 0), ("1, 1", 1)])[1];
    assert_eq!(alone, with_friend);
}

#[test]
fn bad_mixes_and_picks_are_refused_with_a_reason() {
    let err = |mix: &str, pick: u32| {
        evaluate(&format!("t = terrain.tile@1(size=[64, 64], cell=2, seed=4)\nb = solid.box@1(size=[1, 1, 1])\nf = scatter.place@1(b, t, mix=\"{mix}\", pick={pick})\noutput f = f\n")).unwrap_err()
    };
    assert!(err("0.5, x", 0).contains("scatter.bad_mix"));
    assert!(err("0.5, -1", 0).contains("scatter.bad_mix"));
    assert!(err("1, 1", 2).contains("scatter.bad_pick"));
}
