//! G4c sharing, preflight, explicit realization and reflection conformance.
#![expect(clippy::unwrap_used, reason = "test program construction")]
use ve_graph::{Outcome, Session, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn document(source: &str) -> ve_graph::Document {
    let parsed = parse_text(&builtin_registry(), source).unwrap();
    let mut session = Session::with_builtin(4);
    let batch = serde_json::json!({"doc":session.document().id,"base_revision":0,
        "transaction":"t_fixture","commands":parsed.commands});
    assert!(matches!(
        session.apply_json(&batch.to_string()),
        Outcome::Committed { .. }
    ));
    session.document().clone()
}

#[test]
fn g4c_01_g4c_02_nested_sets_share_one_prototype() {
    let doc = document(
        "b = solid.box(size=[1,1,1])\na = inst.array(b, count_x=100, spacing=[2,0,0])\ns = inst.repeat(a, count=100, step=[0,2,0])\noutput s",
    );
    let mut evaluator = Evaluator::default();
    evaluator.begin(&doc, Quality::Preview).unwrap();
    assert_eq!(evaluator.evaluated_nodes(), 0);
    let result = evaluator.evaluate(&doc, Quality::Preview).unwrap();
    assert_eq!(result.parts.len(), 1);
    assert_eq!(result.parts[0].indices.len(), 36);
    assert_eq!(result.parts[0].transforms.as_ref().unwrap().len(), 10_000);
    assert!((result.parts[0].volume - 10_000.0).abs() < 1e-6);
    assert_eq!(result.evaluated_nodes, 3);
    let cached = evaluator.evaluate(&doc, Quality::Preview).unwrap();
    assert_eq!(cached.evaluated_nodes, 0);
}

#[test]
fn g4c_03_realization_budget_refuses_before_kernel_work() {
    let cheap = document(
        "b = solid.box(size=[1,1,1])\ns = inst.array(b, count_x=10000, spacing=[2,0,0])\nr = inst.realize(s)\noutput r",
    );
    let mut planning = Evaluator::default();
    planning.begin(&cheap, Quality::Preview).unwrap();
    assert_eq!(
        planning.evaluated_nodes(),
        0,
        "ten thousand boxes fit predicted realization limits"
    );
    let doc = document(
        "b = solid.sphere(radius=1, segments=16)\ns = inst.array(b, count_x=10000, spacing=[2,0,0])\nr = inst.realize(s)\noutput r",
    );
    let mut evaluator = Evaluator::default();
    let error = evaluator.begin(&doc, Quality::Preview).unwrap_err();
    assert_eq!(error.code, "budget.triangles");
    assert_eq!(evaluator.evaluated_nodes(), 0);
}

#[test]
fn g4c_01_group_prototype_and_radial_set_share_evaluation() {
    let doc = document(
        "group block\n b = solid.box(size=[1,1,1])\n result b\nend\np = block()\ns = inst.radial(p, count=4, radius=2)\noutput s",
    );
    let result = Evaluator::default()
        .evaluate(&doc, Quality::Preview)
        .unwrap();
    assert_eq!(result.evaluated_nodes, 2);
    assert_eq!(result.parts[0].transforms.as_ref().unwrap().len(), 4);
    assert!((result.parts[0].volume - 4.0).abs() < 1e-6);
}

#[test]
fn g4c_draw_work_budget_counts_placements_before_geometry() {
    // What the placements would draw is bounded in preflight, before any node runs: a 32,000-triangle sphere ten thousand
    // times is over the preview drawn limit.
    let doc = document(
        "b = solid.sphere(radius=1, segments=256)\ns = inst.repeat(b, count=10000, step=[2,0,0])\noutput s",
    );
    let mut evaluator = Evaluator::default();
    assert_eq!(
        evaluator.begin(&doc, Quality::Preview).unwrap_err().code,
        "budget.drawn_triangles"
    );
    assert_eq!(evaluator.evaluated_nodes(), 0);
}

#[test]
fn g4c_placed_copies_count_once_against_the_stored_budget() {
    // A light shape placed many times is fine: its mesh is stored once (see ADR 0053), whatever the copies add up to.
    let doc = document(
        "b = solid.sphere(radius=1, segments=24)\ns = inst.repeat(b, count=10000, step=[2,0,0])\noutput s",
    );
    let result = Evaluator::default()
        .evaluate(&doc, Quality::Preview)
        .unwrap();
    assert_eq!(result.parts[0].transforms.as_ref().unwrap().len(), 10000);
}

#[test]
fn g4c_04_realization_equals_placed_union() {
    let doc = document(
        "b = solid.box(size=[1,1,1])\ns = inst.repeat(b, count=3, step=[0.5,0,0])\nr = inst.realize(s)\noutput r",
    );
    let result = Evaluator::default().evaluate(&doc, Quality::Final).unwrap();
    assert!((result.parts[0].volume - 2.0).abs() < 1e-6);
}

#[test]
fn g4c_05_reflected_sets_keep_positive_render_matrices() {
    let doc = document(
        "b = solid.box(size=[1,1,1])\nm = xform.move(b, by=[2,0,0])\ns = inst.repeat(m, count=2, step=[0,2,0])\nr = inst.mirror(s, axis=\"x\")\noutput r",
    );
    let result = Evaluator::default().evaluate(&doc, Quality::Final).unwrap();
    assert_eq!(result.parts.len(), 2);
    for part in &result.parts {
        assert!(part.volume > 0.0);
        for matrix in part.transforms.as_ref().unwrap() {
            assert!(glam::Mat4::from_cols_array(matrix).determinant() > 0.0);
        }
    }
    let mut indices: Vec<_> = result
        .parts
        .iter()
        .flat_map(|p| p.instance_indices.iter().copied())
        .collect();
    indices.sort_unstable();
    assert_eq!(indices, vec![0, 1, 2, 3]);
}

#[test]
fn g4c_07_products_are_bounded_without_wraparound() {
    let doc = document(
        "b = solid.box(size=[1,1,1])\ns = inst.array(b, count_x=100000, count_y=100000, count_z=100000, spacing=[1,1,1])\noutput s",
    );
    let mut evaluator = Evaluator::default();
    let error = evaluator.begin(&doc, Quality::Final).unwrap_err();
    assert_eq!(error.code, "budget.instances");
    assert_eq!(error.limit, Some(16_384.0));
    assert!(error.observed.unwrap() > 16_384.0);
    assert_eq!(evaluator.evaluated_nodes(), 0);
}
