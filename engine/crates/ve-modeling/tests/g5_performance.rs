//! Local evidence for the largest supported loft section count; timings are observations, not limits.
use serde_json::{Value, json};
use std::time::Instant;
use ve_graph::{Outcome, Session};
use ve_modeling::{Evaluator, Quality};

#[test]
fn g5_64_section_loft_records_quality_triangle_counts_and_timings() {
    let mut session = Session::with_builtin(3);
    let mut commands = Vec::<Value>::new();
    for index in 0_u8..64 {
        let alias = format!("$section{index}");
        commands.push(json!({"add_node": {"op": "profile.circle@1", "params": {
            "radius": 0.1 + f64::from(index) * 0.0005, "segments": 32,
        }, "alias": alias}}));
    }
    commands.push(
        json!({"add_node": {"op": "solid.loft@1", "params": {"spacing": 0.01}, "alias": "$loft"}}),
    );
    for index in 0_u8..64 {
        commands.push(
            json!({"connect": {"from": {"node": format!("$section{index}"), "port": "out"},
            "to": {"node": "$loft", "port": "sections"}}}),
        );
    }
    commands
        .push(json!({"set_output": {"name": "loft", "from": {"node": "$loft", "port": "out"}}}));
    let batch = json!({"doc": session.document().id, "base_revision": 0,
        "transaction": "t_g5_bench", "commands": commands});
    assert!(matches!(
        session.apply_json(&batch.to_string()),
        Outcome::Committed { .. }
    ));

    for quality in [Quality::Preview, Quality::Final] {
        let start = Instant::now();
        let result = Evaluator::default()
            .evaluate(session.document(), quality)
            .unwrap();
        let elapsed = start.elapsed();
        assert_eq!(result.parts.len(), 1);
        let triangles = result.parts[0].indices.len() / 3;
        println!("64-section {quality:?}: {triangles} triangles, {elapsed:?}");
    }
}

#[test]
fn g5_31_five_chained_unions_of_overlapping_boxes_fit_preview() {
    let mut session = Session::with_builtin(2);
    let mut commands = Vec::<Value>::new();
    let mut previous = String::new();
    for round in 0..5 {
        let mut aliases = Vec::new();
        if round > 0 {
            aliases.push(previous.clone());
        }
        for index in 0..13 {
            let alias = format!("$box_{round}_{index}");
            commands.push(json!({"add_node": {"op": "solid.box@1", "params": {
                "size": [1.0, 1.0, 1.0],
            }, "alias": alias}}));
            aliases.push(alias);
        }
        let union = format!("$union_{round}");
        commands.push(json!({"add_node": {"op": "solid.union@1", "alias": union}}));
        for alias in aliases {
            commands.push(json!({"connect": {"from": {"node": alias, "port": "out"},
                "to": {"node": union, "port": "in"}}}));
        }
        previous = union;
    }
    commands.push(json!({"set_output": {"name": "result", "from": {
        "node": previous, "port": "out",
    }}}));
    let batch = json!({"doc": session.document().id, "base_revision": 0,
        "transaction": "t_g5_union_chain", "commands": commands});
    assert!(matches!(
        session.apply_json(&batch.to_string()),
        Outcome::Committed { .. }
    ));

    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .expect("conservative estimates must allow the small overlapping-box result");
    assert_eq!(result.parts.len(), 1);
    assert_eq!(result.parts[0].indices.len() / 3, 12);
}
