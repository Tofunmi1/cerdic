//! G3-21: every one of the 53 spike programs is classified, and the ones that translate run in the
//! engine and agree with the spike evaluator's volume and size.
use serde_json::Value;
use std::path::Path;
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn size(positions: &[[f32; 3]]) -> [f64; 3] {
    let mut out = [0.0; 3];
    for (axis, slot) in out.iter_mut().enumerate() {
        let values = positions.iter().map(|p| f64::from(p[axis]));
        *slot = values.clone().fold(f64::MIN, f64::max) - values.fold(f64::MAX, f64::min);
    }
    out
}

#[test]
fn g3_21_all_53_programs_are_classified_and_the_runnable_ones_match_the_reference() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/spike53");
    let index: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string(dir.join("index.json")).unwrap()).unwrap();
    assert_eq!(index.len(), 53);
    let registry = builtin_registry();
    let mut ran = 0;
    for row in &index {
        let (id, status) = (row["id"].as_str().unwrap(), row["status"].as_str().unwrap());
        assert!(
            ["supported", "changed", "deferred"].contains(&status),
            "{id}"
        );
        assert!(
            !row["reason"].as_str().unwrap().is_empty(),
            "{id} needs a reason"
        );
        if status == "deferred" {
            continue;
        }
        let text = std::fs::read_to_string(dir.join(format!("{id}.txt"))).unwrap();
        let batch = parse_text(&registry, &text).unwrap_or_else(|e| panic!("{id}: {e:?}"));
        let mut session = Session::with_builtin(9);
        let outcome = session.apply(&CommandBatch {
            doc: session.document().id.clone(),
            base_revision: 0,
            transaction: TransactionId::parse("t_spike").unwrap(),
            label: None,
            commands: batch.commands,
        });
        assert!(
            matches!(outcome, Outcome::Committed { .. }),
            "{id}: {outcome:?}"
        );
        let result = Evaluator::default()
            .evaluate(session.document(), Quality::Final)
            .unwrap_or_else(|e| panic!("{id}: {} {}", e.code, e.message));
        let part = &result.parts[0];
        let reference = row["volume"].as_f64().unwrap();
        assert!(
            (part.volume - reference).abs() <= 0.015 * reference.abs().max(1e-9),
            "{id}: volume {} against {reference}",
            part.volume
        );
        let want: Vec<f64> = row["size"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect();
        for (got, want) in size(&part.positions).iter().zip(&want) {
            assert!(
                (got - want).abs() <= 0.002,
                "{id}: size {got} against {want}"
            );
        }
        ran += 1;
    }
    assert!(ran >= 20, "{ran} programs ran");
}
