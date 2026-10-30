//! `inst.place` puts copies of a shape at positions you list (a jar here, a sack there), as one shared set: the prototype is stored
//! once however many copies there are. Written before the body; do not weaken.
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
        transaction: TransactionId::parse("t_g19_place").unwrap(),
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

/// Where a column-major placement puts the origin.
fn origin(m: &[f32; 16]) -> [f32; 3] {
    [m[12], m[13], m[14]]
}

#[test]
fn copies_stand_where_they_are_listed() {
    let result = evaluate("b = solid.box@1(size=[1, 1, 1])\np = inst.place@1(b, at=[[0, 0, 0], [3, 0, 1], [-2, 1, 4]])\noutput p = p\n").unwrap();
    let part = &result.parts[0];
    let placed = part.transforms.as_ref().unwrap();
    assert_eq!(placed.len(), 3);
    assert_eq!(origin(&placed[0]), [0.0, 0.0, 0.0]);
    assert_eq!(origin(&placed[1]), [3.0, 0.0, 1.0]);
    assert_eq!(origin(&placed[2]), [-2.0, 1.0, 4.0]);
    assert_eq!(
        part.indices.len() / 3,
        12,
        "the box is stored once, not once per copy"
    );
}

#[test]
fn each_copy_may_turn_and_scale_on_its_own() {
    // yaw_scale gives [yaw in degrees about the vertical axis, uniform scale] for each position.
    let result = evaluate(
        "b = solid.box@1(size=[1, 1, 1])\np = inst.place@1(b, at=[[0, 0, 0], [5, 0, 0]], yaw_scale=[[90, 1], [90, 2]])\noutput p = p\n",
    )
    .unwrap();
    let placed = result.parts[0].transforms.as_ref().unwrap();
    // Column-major: the first column is where +X goes. A 90 degree turn about Y sends +X to -Z; scale 2 doubles its length.
    let (first, second) = (&placed[0], &placed[1]);
    assert!(
        (first[0]).abs() < 1e-5 && (first[2] + 1.0).abs() < 1e-5,
        "the first copy is turned a quarter: {:?}",
        &first[..3]
    );
    assert!(
        (second[0]).abs() < 1e-5 && (second[2] + 2.0).abs() < 1e-5,
        "the second is turned and twice as big: {:?}",
        &second[..3]
    );
    assert_eq!(origin(second), [5.0, 0.0, 0.0]);
}

#[test]
fn a_list_of_turns_must_match_the_list_of_positions() {
    let error = evaluate("b = solid.box@1(size=[1, 1, 1])\np = inst.place@1(b, at=[[0, 0, 0], [5, 0, 0]], yaw_scale=[[90, 1]])\noutput p = p\n").unwrap_err();
    assert!(
        error.contains("yaw_scale") && error.contains('2'),
        "the message says what to match: {error}"
    );
}

#[test]
fn a_placed_set_can_be_joined_planted_and_given_a_material() {
    let result = evaluate(
        "b = solid.box@1(size=[1, 1, 1])\np = inst.place@1(b, at=[[0, 0, 0], [3, 0, 0]])\nm = solid.material@1(p, color=[1, 0, 0])\noutput m = m\n",
    )
    .unwrap();
    assert_eq!(result.parts[0].transforms.as_ref().unwrap().len(), 2);
    assert!(result.parts[0].material.is_some());
}
