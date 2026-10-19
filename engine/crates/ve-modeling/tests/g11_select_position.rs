//! `poly.select` by position: `by="height"` (a coordinate along an axis) and `by="extreme"` (the highest or lowest elements).
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
        transaction: TransactionId::parse("t_g11_select_position").unwrap(),
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

fn volume(text: &str) -> f64 {
    evaluate(text)
        .unwrap_or_else(|(code, message)| panic!("{code}: {message}"))
        .parts[0]
        .volume
}

fn refused(text: &str) -> String {
    evaluate(text).err().map(|(code, _)| code).unwrap()
}

const BOX: &str = "box = solid.box@1(size=[1, 1, 1])\n\
    triangles = poly.from_solid@1(box)\n\
    mesh = poly.merge_coplanar@1(triangles)\n";

fn pull_up(selection: &str) -> String {
    format!(
        "{BOX}{selection}\
         up = poly.extrude_faces@1(mesh, picked, offset=[0, 0.5, 0])\n\
         solid = poly.to_solid@1(up)\n\
         output solid"
    )
}

#[test]
fn the_top_face_is_the_extreme_face_along_y() {
    let top = pull_up(
        "picked = poly.select@1(mesh, by=\"extreme\", axis=\"y\", side=\"max\", expect=\"one\")\n",
    );
    assert!((volume(&top) - 1.5).abs() < 1.0e-6);
}

#[test]
fn the_bottom_face_is_the_extreme_face_with_side_min() {
    // Pushing the bottom face upward by 0.5 would fold it into the box, so push it down through the offset instead.
    let text = format!(
        "{BOX}picked = poly.select@1(mesh, by=\"extreme\", axis=\"y\", side=\"min\", expect=\"one\")\n\
         down = poly.extrude_faces@1(mesh, picked, offset=[0, -0.5, 0])\n\
         solid = poly.to_solid@1(down)\n\
         output solid"
    );
    assert!((volume(&text) - 1.5).abs() < 1.0e-6);
}

#[test]
fn height_picks_the_face_at_that_coordinate() {
    let top = pull_up(
        "picked = poly.select@1(mesh, by=\"height\", axis=\"y\", height=0.5, tolerance=0.01, expect=\"one\")\n",
    );
    assert!((volume(&top) - 1.5).abs() < 1.0e-6);
}

#[test]
fn height_on_the_side_faces_picks_the_right_one() {
    // The face whose centre is at x = 0.5.
    let text = format!(
        "{BOX}picked = poly.select@1(mesh, by=\"height\", axis=\"x\", height=0.5, tolerance=0.01, expect=\"one\")\n\
         out = poly.extrude_faces@1(mesh, picked, offset=[0.5, 0, 0])\n\
         solid = poly.to_solid@1(out)\n\
         output solid"
    );
    assert!((volume(&text) - 1.5).abs() < 1.0e-6);
}

#[test]
fn the_four_top_edges_bevel_as_the_extreme_edges() {
    let text = format!(
        "{BOX}top = poly.select@1(mesh, element=\"edge\", by=\"extreme\", axis=\"y\", side=\"max\")\n\
         b = poly.bevel_network@1(mesh, top, width=0.1, segments=3)\n\
         solid = poly.to_solid@1(b)\n\
         output solid"
    );
    // The same volume window as the explicit box selection of the four top edges.
    let v = volume(&text);
    assert!(v < 0.995 && v > 0.97, "volume {v}");
}

#[test]
fn several_extreme_edges_are_ambiguous_when_one_is_expected() {
    let code = refused(&format!(
        "{BOX}top = poly.select@1(mesh, element=\"edge\", by=\"extreme\", axis=\"y\", side=\"max\", expect=\"one\")\n\
         b = poly.bevel_network@1(mesh, top, width=0.1, segments=2)\n\
         solid = poly.to_solid@1(b)\n\
         output solid"
    ));
    assert_eq!(code, "selection.ambiguous");
}

#[test]
fn a_height_with_nothing_there_is_lost() {
    let code = refused(&format!(
        "{BOX}picked = poly.select@1(mesh, element=\"vertex\", by=\"height\", axis=\"y\", height=7, tolerance=0.01)\n\
         up = poly.extrude_faces@1(mesh, picked, offset=[0, 0.5, 0])\n\
         solid = poly.to_solid@1(up)\n\
         output solid"
    ));
    assert_eq!(code, "selection.lost");
}
