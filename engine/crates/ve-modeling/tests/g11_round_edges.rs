//! `poly.round_edges`: one node that rounds every hard edge, equal to select-by-sharp followed by `poly.bevel_network`.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "the helper unwraps fixtures that are known to be valid and reports failures by panicking"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(text: &str) -> Result<Evaluation, (String, String)> {
    let batch = parse_text(&builtin_registry(), text).unwrap();
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g11_round_edges").unwrap(),
        label: None,
        commands: batch.commands,
    });
    assert!(
        matches!(outcome, Outcome::Committed { .. }),
        "commits: {outcome:?}"
    );
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message))
}

fn ok(text: &str) -> Evaluation {
    evaluate(text).unwrap_or_else(|(code, message)| panic!("{code}: {message}"))
}

const BOX: &str = "box = solid.box@1(size=[1, 1, 1])\n\
    triangles = poly.from_solid@1(box)\n\
    mesh = poly.merge_coplanar@1(triangles)\n";

#[test]
fn one_node_equals_select_then_bevel() {
    let long = ok(&format!(
        "{BOX}sharp = poly.select@1(mesh, element=\"edge\", by=\"sharp\", angle=0.5)\n\
         b = poly.bevel_network@1(mesh, sharp, width=0.1, segments=4)\n\
         solid = poly.to_solid@1(b)\n\
         output solid"
    ));
    let short = ok(&format!(
        "{BOX}b = poly.round_edges@1(mesh, width=0.1, segments=4)\n\
         solid = poly.to_solid@1(b)\n\
         output solid"
    ));
    let (a, b) = (&long.parts[0], &short.parts[0]);
    assert!(
        (a.volume - b.volume).abs() < 1.0e-12,
        "{} {}",
        a.volume,
        b.volume
    );
    assert_eq!(a.indices.len(), b.indices.len());
    assert!((b.volume - 0.973_285_987_320_342).abs() < 1.0e-12);
}

#[test]
fn a_larger_width_removes_more_volume() {
    let volume = |width: &str| {
        ok(&format!(
            "{BOX}b = poly.round_edges@1(mesh, width={width}, segments=3)\n\
             solid = poly.to_solid@1(b)\n\
             output solid"
        ))
        .parts[0]
            .volume
    };
    assert!(volume("0.2") < volume("0.1"));
    assert!(volume("0.1") < 1.0);
}

#[test]
fn a_flat_mesh_has_no_hard_edge_and_says_what_to_do() {
    let (code, message) = evaluate(
        "grid = poly.grid@1(width=1, height=1, columns=3, rows=3)\n\
         b = poly.round_edges@1(grid, width=0.05, segments=2)\n\
         solid = poly.to_solid@1(b)\n\
         output solid",
    )
    .err()
    .unwrap();
    assert_eq!(code, "selection.lost");
    assert!(message.contains("lower angle"), "{message}");
}

#[test]
fn unmerged_triangles_round_every_triangle_edge_hint_is_in_the_docs() {
    // The operator documentation tells a model to merge coplanar triangles first.
    let registry = builtin_registry();
    let id = ve_graph::OpId::new("poly.round_edges", 1).unwrap();
    let spec = registry.get(&id).unwrap();
    assert!(spec.doc.contains("poly.merge_coplanar"));
}
