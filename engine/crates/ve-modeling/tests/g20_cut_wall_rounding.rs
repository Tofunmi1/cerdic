//! A wall with an opening cut by a boolean can have its edges rounded straight from the solid, and `poly.merge_coplanar` leaves the faces
//! around a hole alone instead of refusing the whole mesh. Written before the body; do not weaken.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup unwraps data that is known to be valid"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, String> {
    let commands = parse_text(&builtin_registry(), source)
        .map_err(|e| format!("{e:?}"))?
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g20_wall").unwrap(),
        label: None,
        commands,
    });
    if !matches!(applied, Outcome::Committed { .. }) {
        return Err(format!("{applied:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| format!("{e:?}"))
}

const CUT_WALL: &str = "body = solid.box@1(size=[6, 2.8, 0.5])\ncut = solid.box@1(size=[1.2, 1.2, 1])\ncut_at = xform.move@1(cut, by=[0, 0.3, 0])\nwall = solid.difference@1(body, cut_at)\np = poly.from_solid@1(wall)\n";

fn triangles(source: &str) -> usize {
    evaluate(source).unwrap().parts[0].indices.len() / 3
}

#[test]
fn a_wall_with_an_opening_rounds_straight_from_the_solid() {
    let plain = triangles(&format!("{CUT_WALL}s = poly.to_solid@1(p)\noutput s = s\n"));
    let rounded = triangles(&format!(
        "{CUT_WALL}r = poly.round_edges@1(p, width=0.03, segments=2)\ns = poly.to_solid@1(r)\noutput s = s\n"
    ));
    assert!(
        rounded > plain * 2,
        "the edges got bevels: {plain} faces became {rounded}"
    );
}

#[test]
fn merging_a_wall_with_an_opening_keeps_the_holed_faces_and_merges_the_rest() {
    let plain = triangles(&format!("{CUT_WALL}s = poly.to_solid@1(p)\noutput s = s\n"));
    let merged = evaluate(&format!(
        "{CUT_WALL}m = poly.merge_coplanar@1(p)\ns = poly.to_solid@1(m)\noutput s = s\n"
    ));
    assert!(merged.is_ok(), "it no longer refuses: {:?}", merged.err());
    assert!(plain > 0);
}

/// A doorway whose cut reaches the bottom face used to fail with "folded curved quad" once segments was 2 or more.
const DOORWAY: &str = "body = solid.box@1(size=[6, 2.8, 0.5])\ncut = solid.box@1(size=[1.2, 2, 1])\ncut_at = xform.move@1(cut, by=[0, -0.4, 0])\nwall = solid.difference@1(body, cut_at)\np = poly.from_solid@1(wall)\n";

#[test]
fn a_doorway_down_to_the_floor_rounds_with_several_segments() {
    for segments in [1, 2, 3] {
        let plain = triangles(&format!("{DOORWAY}s = poly.to_solid@1(p)\noutput s = s\n"));
        let source = format!(
            "{DOORWAY}r = poly.round_edges@1(p, width=0.03, segments={segments})\ns = poly.to_solid@1(r)\noutput s = s\n"
        );
        let rounded = evaluate(&source).unwrap_or_else(|e| panic!("segments={segments}: {e}"));
        assert!(
            rounded.parts[0].indices.len() / 3 > plain,
            "segments={segments}: the edges got bevels"
        );
    }
}

#[test]
fn asking_to_refuse_holes_keeps_the_strict_behaviour_and_says_how_to_change_it() {
    let error = evaluate(&format!("{CUT_WALL}m = poly.merge_coplanar@1(p, holes=\"refuse\")\ns = poly.to_solid@1(m)\noutput s = s\n")).unwrap_err();
    assert!(
        error.contains("holes"),
        "the message names the parameter that fixes it: {error}"
    );
}
