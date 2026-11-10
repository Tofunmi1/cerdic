//! `rig.morph` blends equal-topology edits and exports their deltas.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "small authored graph fixtures are known to be valid"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|error| format!("{error:?}"))?
        .commands;
    let mut session = Session::with_builtin(23);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_rig_morph").unwrap(),
        label: None,
        commands,
    });
    if !matches!(applied, Outcome::Committed { .. }) {
        return Err(format!("{applied:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|error| format!("{error:?}"))
}

#[test]
fn a_sculpted_shape_key_blends_and_keeps_its_glb_delta() {
    let result = evaluate(
        "base = poly.grid@1(width=2, height=2, columns=12, rows=12)\n\
         target = poly.sculpt@1(base, strokes=\"inflate 0 0 0 0.5 0.2\", falloff=\"surface\")\n\
         half = rig.morph@1(base, target, name=\"brow_raise\", weight=0.5)\n\
         output neutral = base\noutput half = half\n",
    )
    .unwrap();
    let neutral = result.parts.iter().find(|part| part.name == "neutral").unwrap();
    let half = result.parts.iter().find(|part| part.name == "half").unwrap();
    let target = half.morph_targets.first().expect("shape key metadata");
    assert_eq!(target.name, "brow_raise");
    assert_eq!(target.weight, 0.5);
    assert_eq!(target.positions.len(), half.positions.len());
    let moved = neutral
        .positions
        .iter()
        .zip(&half.positions)
        .map(|(a, b)| (a[2] - b[2]).abs())
        .fold(0.0_f32, f32::max);
    assert!(moved > 0.02, "the current key weight deforms the mesh: {moved}");
}

#[test]
fn shape_keys_refuse_different_topology_with_a_repair_hint() {
    let error = evaluate(
        "a = poly.grid@1(width=2, height=2, columns=8, rows=8)\n\
         b = poly.grid@1(width=2, height=2, columns=9, rows=8)\n\
         m = rig.morph@1(a, b, name=\"bad\")\noutput m = m\n",
    )
    .unwrap_err();
    assert!(error.contains("rig.morph_topology"), "{error}");
    assert!(error.contains("duplicating the base"), "{error}");
}
