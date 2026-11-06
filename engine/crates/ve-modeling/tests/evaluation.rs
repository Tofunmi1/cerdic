//! The first worker workload and its resource/cache invariants.
use ve_graph::{Document, Outcome, Session};
use ve_modeling::{Evaluator, Quality};

fn fixture() -> Result<Document, Box<dyn std::error::Error>> {
    let mut session = Session::with_builtin(1);
    let batch = serde_json::json!({"doc":session.document().id,"base_revision":0,"transaction":"t_fixture",
    "commands":[
        {"add_node":{"op":"solid.box@1","params":{"size":[1,1,1]},"alias":"$box"}},
        {"add_node":{"op":"solid.sphere@1","params":{"radius":0.6,"segments":24},"alias":"$ball"}},
        {"add_node":{"op":"solid.difference@1","alias":"$cut"}},
        {"connect":{"from":{"node":"$box","port":"out"},"to":{"node":"$cut","port":"in"}}},
        {"connect":{"from":{"node":"$ball","port":"out"},"to":{"node":"$cut","port":"in"}}},
        {"set_output":{"name":"cut","from":{"node":"$cut","port":"out"}}}
    ]});
    if !matches!(
        session.apply_json(&batch.to_string()),
        Outcome::Committed { .. }
    ) {
        return Err("fixture must commit".into());
    }
    Ok(session.document().clone())
}

#[test]
fn g2_01_primitive_boolean_matches_direct_kernel() {
    let doc = fixture().unwrap();
    let mut evaluator = Evaluator::default();
    let result = evaluator.evaluate(&doc, Quality::Preview).unwrap();
    let direct = ve_kernel::Solid::cube([1.0; 3], true)
        .unwrap()
        .difference(&ve_kernel::Solid::sphere(0.6, 24).unwrap())
        .unwrap();
    assert!((result.parts[0].volume - direct.volume()).abs() < 1e-6);
    assert_eq!(result.parts[0].node, doc.outputs[0].from.node);
    assert!(!result.parts[0].positions.is_empty());
}

#[test]
fn a_draft_shows_the_output_growing_one_node_at_a_time() {
    let doc = fixture().unwrap();
    let mut evaluator = Evaluator::default();
    evaluator.begin(&doc, Quality::Preview).unwrap();
    // The fixture runs box, sphere, then the difference. After the first node the output already has a draft: the bare box.
    assert!(evaluator.step().unwrap().is_none());
    let first = evaluator.take_drafts();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].node, doc.outputs[0].from.node);
    let box_volume = first[0].volume;
    // Nothing changed since: nothing to take.
    assert!(evaluator.take_drafts().is_empty());
    // After the sphere and the difference the same output has a newer draft, with the sphere cut out of the box.
    let mut done = None;
    let mut latest = Vec::new();
    while done.is_none() {
        done = evaluator.step().unwrap();
        let drafts = evaluator.take_drafts();
        if !drafts.is_empty() {
            latest = drafts;
        }
    }
    assert_eq!(latest.len(), 1);
    assert!(latest[0].volume < box_volume);
    // The finished scene has the same part.
    let finished = done.unwrap();
    assert!((finished.parts[0].volume - latest[0].volume).abs() < 1e-6);
}

#[test]
fn g2_09_cache_eviction_obeys_byte_cap() {
    let doc = fixture().unwrap();
    let mut evaluator = Evaluator::with_cache_limit(1024);
    evaluator.evaluate(&doc, Quality::Preview).unwrap();
    assert!(evaluator.cache_bytes() <= 1024);
}

#[test]
fn g2_10_a_heavy_primitive_is_no_longer_refused_by_a_triangle_limit() {
    // The preview triangle limit (200,000) was removed on 2026-11-06: a half-million-triangle sphere is built, not refused. What still bounds
    // a document is its memory (`budget.memory`) and the time the evaluation takes.
    let mut doc = fixture().unwrap();
    doc.nodes[1]
        .params
        .insert("segments".into(), ve_graph::Value::Int(512));
    let mut evaluator = Evaluator::default();
    let result = evaluator.evaluate(&doc, Quality::Preview).unwrap();
    // The sphere alone is 524,288 triangles; the box cut by it keeps a good share of them.
    assert!(result.parts[0].indices.len() / 3 > 50_000);
    assert!(evaluator.evaluated_nodes() > 0);
}

#[test]
fn g2_10_unknown_schema_or_units_refused_before_kernel_work() {
    let mut doc = fixture().unwrap();
    let mut evaluator = Evaluator::default();
    doc.schema = 99;
    assert_eq!(
        evaluator.evaluate(&doc, Quality::Preview).unwrap_err().code,
        "store.newer_version"
    );
    doc.schema = ve_graph::SCHEMA;
    doc.units = "cm".into();
    assert_eq!(
        evaluator.evaluate(&doc, Quality::Preview).unwrap_err().code,
        "kernel.invalid_input"
    );
    assert_eq!(evaluator.evaluated_nodes(), 0);
}

#[test]
fn g2_15_regeneration_reuses_cache_and_stays_bounded() {
    let doc = fixture().unwrap();
    let mut evaluator = Evaluator::default();
    let first = evaluator.evaluate(&doc, Quality::Preview).unwrap();
    let bytes = evaluator.cache_bytes();
    for _ in 0..100 {
        let next = evaluator.evaluate(&doc, Quality::Preview).unwrap();
        assert_eq!(next.parts[0].positions, first.parts[0].positions);
        assert_eq!(next.evaluated_nodes, 0);
        assert_eq!(evaluator.cache_bytes(), bytes);
    }
}

#[test]
fn g2_08_partial_cancel_never_publishes_staged_cache_values() {
    let mut evaluator = Evaluator::default();
    evaluator
        .begin(&fixture().unwrap(), Quality::Preview)
        .unwrap();
    assert!(evaluator.step().unwrap().is_none());
    assert_eq!(evaluator.evaluated_nodes(), 1);
    evaluator.cancel();
    assert_eq!(evaluator.cache_bytes(), 0);
}
