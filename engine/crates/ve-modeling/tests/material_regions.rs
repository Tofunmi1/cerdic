//! Booleans and moves keep each input's material on its own faces.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "the helper unwraps fixtures that are known to be valid and reports failures by panicking"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Part, Quality};

fn evaluate(text: &str) -> Evaluation {
    let batch = parse_text(&builtin_registry(), text).unwrap();
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_regions").unwrap(),
        label: None,
        commands: batch.commands,
    });
    assert!(
        matches!(outcome, Outcome::Committed { .. }),
        "commits: {outcome:?}"
    );
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap_or_else(|e| panic!("{}: {}", e.code, e.message))
}

fn red(part: &Part) -> bool {
    part.material.as_ref().is_some_and(|m| m.color[0] > 0.5)
}

fn mean_x(part: &Part) -> f32 {
    let sum: f32 = part.positions.iter().map(|p| p[0]).sum();
    sum / f32::from(u16::try_from(part.positions.len()).unwrap())
}

const TWO_BOXES: &str = "a = solid.box@1(size=[1, 1, 1])\n\
    left = xform.move@1(a, by=[-0.4, 0, 0])\n\
    right = xform.move@1(a, by=[0.4, 0, 0])\n\
    red = solid.material@1(left, color=[0.9, 0.1, 0.1])\n\
    blue = solid.material@1(right, color=[0.1, 0.1, 0.9])\n";

#[test]
fn a_union_of_two_colours_is_one_solid_drawn_in_two_colours() {
    let result = evaluate(&format!(
        "{TWO_BOXES}both = solid.union@1(red, blue)\noutput both"
    ));
    assert_eq!(result.parts.len(), 2, "one part per colour");
    let (first, second) = (&result.parts[0], &result.parts[1]);
    let (r, b) = if red(first) {
        (first, second)
    } else {
        (second, first)
    };
    assert!(red(r) && !red(b));
    assert!(
        mean_x(r) < -0.1 && mean_x(b) > 0.1,
        "each colour is on its own box"
    );
    let volume = r.volume + b.volume;
    assert!(
        (volume - 1.8).abs() < 1.0e-4,
        "the shares add up to the joined volume: {volume}"
    );
}

#[test]
fn subdivision_keeps_material_regions_on_their_child_faces() {
    let result = evaluate(&format!(
        "{TWO_BOXES}both = solid.union@1(red, blue)\n\
        cage = poly.from_solid@1(both)\n\
        smooth = poly.subdivide@1(cage, levels=1)\n\
        output smooth"
    ));
    assert_eq!(result.parts.len(), 2);
    assert!(result.parts.iter().any(red));
    assert!(result.parts.iter().any(|part| !red(part)));
}

#[test]
fn a_move_keeps_every_colour() {
    let result = evaluate(&format!(
        "{TWO_BOXES}both = solid.union@1(red, blue)\nmoved = xform.move@1(both, by=[10, 0, 0])\noutput moved"
    ));
    assert_eq!(result.parts.len(), 2);
    assert!(result.parts.iter().all(|p| mean_x(p) > 8.0));
    assert!(result.parts.iter().any(red) && result.parts.iter().any(|p| !red(p)));
}

fn pattern_keeps_both_colours(pattern: &str) {
    let result = evaluate(&format!(
        "{TWO_BOXES}both = solid.union@1(red, blue)\n\
         shifted = xform.move@1(both, by=[4, 0, 0])\n\
         copies = {pattern}\n\
         output copies"
    ));
    assert!(
        result.parts.iter().any(red) && result.parts.iter().any(|p| !red(p)),
        "{pattern}: {} parts, both colours expected",
        result.parts.len()
    );
}

#[test]
fn mirror_keeps_every_colour() {
    pattern_keeps_both_colours("xform.mirror@1(shifted, axis=\"x\")");
}

#[test]
fn array_keeps_every_colour() {
    pattern_keeps_both_colours("xform.array@1(shifted, count_z=3, spacing=[0, 0, 3])");
}

#[test]
fn coincident_copies_do_not_break_the_colours() {
    // Three copies on top of each other (no spacing along the counted axis) fuse into one.
    pattern_keeps_both_colours("xform.array@1(shifted, count_x=3, spacing=[0, 0, 3])");
}

#[test]
fn radial_keeps_every_colour() {
    pattern_keeps_both_colours("xform.radial@1(shifted, count=4, radius=0)");
}

#[test]
fn repeat_keeps_every_colour() {
    pattern_keeps_both_colours(
        "xform.repeat@1(shifted, count=3, step=[0, 2, 0], rotate_step=[0, 0, 0])",
    );
}

#[test]
fn a_cutter_without_a_colour_leaves_the_body_one_colour() {
    let result = evaluate(
        "body = solid.box@1(size=[2, 2, 2])\n\
         hole = solid.sphere@1(radius=0.8, segments=32)\n\
         red = solid.material@1(body, color=[0.9, 0.1, 0.1])\n\
         cut = solid.difference@1(red, hole)\n\
         output cut",
    );
    assert_eq!(result.parts.len(), 1);
    assert!(red(&result.parts[0]), "the cavity wall is red too");
}

#[test]
fn a_coloured_cutter_inlays_its_colour_on_the_faces_it_cuts() {
    let result = evaluate(
        "body = solid.box@1(size=[2, 2, 2])\n\
         hole = solid.sphere@1(radius=0.8, segments=32)\n\
         red = solid.material@1(body, color=[0.9, 0.1, 0.1])\n\
         green = solid.material@1(hole, color=[0.1, 0.8, 0.1])\n\
         cut = solid.difference@1(red, green)\n\
         output cut",
    );
    assert_eq!(result.parts.len(), 2, "red outside, green inside");
    let inside = result.parts.iter().find(|p| !red(p)).unwrap();
    let outside = result.parts.iter().find(|p| red(p)).unwrap();
    let reach = |part: &Part| {
        part.positions
            .iter()
            .map(|p| p[0].hypot(p[1]).hypot(p[2]))
            .fold(f32::MAX, f32::min)
    };
    assert!(reach(inside) < 0.85, "the green faces are the cavity wall");
    assert!(reach(outside) > 0.9, "the red faces are the outside");
}

#[test]
fn a_material_on_top_recolours_the_whole_union() {
    let result = evaluate(&format!(
        "{TWO_BOXES}both = solid.union@1(red, blue)\n\
         grey = solid.material@1(both, color=[0.5, 0.5, 0.5])\n\
         output grey"
    ));
    assert_eq!(result.parts.len(), 1);
    assert!((result.parts[0].material.as_ref().unwrap().color[0] - 0.5).abs() < 1.0e-6);
}

#[test]
fn a_set_of_a_two_colour_prototype_draws_each_colour_for_every_placement() {
    let result = evaluate(&format!(
        "{TWO_BOXES}both = solid.union@1(red, blue)\n\
         row = inst.array@1(both, count_x=3, spacing=[4, 0, 0])\n\
         output row"
    ));
    assert_eq!(
        result.parts.len(),
        2,
        "one part per colour, each with three placements"
    );
    assert!(
        result
            .parts
            .iter()
            .all(|p| p.transforms.as_ref().map(Vec::len) == Some(3))
    );
}

#[test]
fn single_colour_and_unpainted_documents_are_unchanged() {
    let plain = evaluate(
        "a = solid.box@1(size=[1, 1, 1])\nb = solid.sphere@1(radius=0.6, segments=24)\n\
         both = solid.union@1(a, b)\noutput both",
    );
    assert_eq!(plain.parts.len(), 1);
    assert!(plain.parts[0].material.is_none());
}
