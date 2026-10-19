//! `poly.bevel_network` (Blender's bevel) through the node language: edge layouts the restricted
//! `poly.bevel_edges` refuses.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "the helper unwraps fixtures that are known to be valid and reports failures by panicking"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn session(text: &str) -> Session {
    let batch = parse_text(&builtin_registry(), text).unwrap();
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g10_bevel_network").unwrap(),
        label: None,
        commands: batch.commands,
    });
    assert!(
        matches!(outcome, Outcome::Committed { .. }),
        "commits: {outcome:?}"
    );
    session
}

fn evaluate(text: &str) -> Result<Evaluation, (String, String)> {
    Evaluator::default()
        .evaluate(session(text).document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message))
}

fn ok(text: &str) -> Evaluation {
    evaluate(text).unwrap_or_else(|(code, message)| panic!("{code}: {message}"))
}

fn refused(text: &str) -> String {
    evaluate(text).err().map(|(code, _)| code).unwrap()
}

const BOX: &str = "box = solid.box@1(size=[1, 1, 1])\n\
    triangles = poly.from_solid@1(box)\n\
    mesh = poly.merge_coplanar@1(triangles)\n";

#[test]
fn the_four_connected_top_edges_bevel_where_bevel_edges_refuses() {
    let selection = "top = poly.select@1(mesh, element=\"edge\", by=\"box\", box_min=[-1, 0.4, -1], box_max=[1, 0.6, 1])\n";
    let old = refused(&format!(
        "{BOX}{selection}b = poly.bevel_edges@1(mesh, top, width=0.1, segments=3)\nsolid = poly.to_solid@1(b)\noutput solid"
    ));
    assert_eq!(old, "poly.bevel_network_unsupported");
    let result = ok(&format!(
        "{BOX}{selection}b = poly.bevel_network@1(mesh, top, width=0.1, segments=3)\nsolid = poly.to_solid@1(b)\noutput solid"
    ));
    let volume = result.parts[0].volume;
    // Four edges of length 1 lose between the round's and the chamfer's cross-section each (0.0021
    // and 0.005 at 0.1), less the overlap at the corners.
    assert!(volume < 0.995 && volume > 0.97, "volume {volume}");
}

#[test]
fn every_edge_of_a_box_rounds_at_once() {
    let result = ok(&format!(
        "{BOX}sharp = poly.select@1(mesh, element=\"edge\", by=\"sharp\", angle=0.5)\n\
         b = poly.bevel_network@1(mesh, sharp, width=0.1, segments=4)\n\
         solid = poly.to_solid@1(b)\n\
         output solid"
    ));
    let volume = result.parts[0].volume;
    assert!(volume < 0.99 && volume > 0.93, "volume {volume}");
    // A closed, sphere-like result: the solid conversion succeeded, so the mesh was a valid manifold.
    assert!(
        result.parts[0].indices.len() / 3 > 250,
        "{}",
        result.parts[0].indices.len() / 3
    );
}

#[test]
fn the_rounded_box_has_the_volume_the_browser_build_measured() {
    // The same program was evaluated by the wasm evaluator in headless Chrome and gave this volume and
    // 300 triangles (the browser packs 152 render vertices, with normals split per face); native must agree.
    let result = ok(&format!(
        "{BOX}sharp = poly.select@1(mesh, element=\"edge\", by=\"sharp\", angle=0.5)\n\
         rounded = poly.bevel_network@1(mesh, sharp, width=0.1, segments=4)\n\
         solid = poly.to_solid@1(rounded)\n\
         output solid"
    ));
    let part = &result.parts[0];
    assert!(
        (part.volume - 0.973_285_987_320_342).abs() < 1.0e-12,
        "{}",
        part.volume
    );
    assert_eq!(part.indices.len() / 3, 300);
}

#[test]
fn a_single_chamfer_matches_the_closed_form() {
    // One edge of a unit box chamfered by 0.2 (one segment) loses w^2 / 2 times the length.
    let result = ok(&format!(
        "{BOX}edge = poly.select@1(mesh, element=\"edge\", by=\"box\", box_min=[-0.1, 0.4, 0.4], box_max=[0.1, 0.6, 0.6], expect=\"one\")\n\
         b = poly.bevel_network@1(mesh, edge, width=0.2, segments=1)\n\
         solid = poly.to_solid@1(b)\n\
         output solid"
    ));
    let volume = result.parts[0].volume;
    assert!((volume - (1.0 - 0.02)).abs() < 1.0e-4, "volume {volume}");
}

#[test]
fn an_empty_selection_and_a_wrong_selection_kind_refuse() {
    let none = refused(&format!(
        "{BOX}none = poly.select@1(mesh, element=\"edge\", by=\"box\", box_min=[5, 5, 5], box_max=[6, 6, 6])\n\
         b = poly.bevel_network@1(mesh, none, width=0.1, segments=2)\n\
         solid = poly.to_solid@1(b)\n\
         output solid"
    ));
    // poly.select refuses an empty result itself, before the bevel runs.
    assert_eq!(none, "selection.lost");
    let faces = refused(&format!(
        "{BOX}faces = poly.select@1(mesh, by=\"normal\", direction=[0, 1, 0], angle=0.05)\n\
         b = poly.bevel_network@1(mesh, faces, width=0.1, segments=2)\n\
         solid = poly.to_solid@1(b)\n\
         output solid"
    ));
    assert_eq!(faces, "poly.selection_kind");
}

#[test]
fn the_result_is_deterministic() {
    let program = format!(
        "{BOX}sharp = poly.select@1(mesh, element=\"edge\", by=\"sharp\", angle=0.5)\n\
         b = poly.bevel_network@1(mesh, sharp, width=0.12, segments=3, profile=0.7, offset=\"width\")\n\
         solid = poly.to_solid@1(b)\n\
         output solid"
    );
    let first = ok(&program);
    let second = ok(&program);
    assert_eq!(
        first.parts[0].volume.to_bits(),
        second.parts[0].volume.to_bits()
    );
    assert_eq!(first.parts[0].indices, second.parts[0].indices);
}
