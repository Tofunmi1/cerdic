//! G11: mirror a mesh across an arbitrary plane, weld the seam, and remain deterministic.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "fixture failures report their evaluation diagnostic"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};
const HALF: &str = "ball = solid.sphere@1(radius=0.5, segments=32)\ntris = poly.from_solid@1(ball)\nhalf = poly.bisect@1(tris, point=[0,0,0], normal=[0,1,0], keep=\"below\", fill=false)\n";
fn evaluate(text: &str) -> Result<Evaluation, (String, Option<String>)> {
    let batch = parse_text(&builtin_registry(), text).unwrap();
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g11_mirror").unwrap(),
        label: None,
        commands: batch.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.suggestion.clone()))
}
fn run(text: &str) -> Evaluation {
    evaluate(text).unwrap_or_else(|(code, suggestion)| panic!("{code}: {suggestion:?}"))
}
fn volume(result: &Evaluation) -> f64 {
    result.parts.iter().map(|part| part.volume).sum()
}
fn bounds(result: &Evaluation, axis: usize) -> [f32; 2] {
    result.parts.iter().flat_map(|part| &part.positions).fold(
        [f32::INFINITY, f32::NEG_INFINITY],
        |mut b, p| {
            b[0] = b[0].min(p[axis]);
            b[1] = b[1].max(p[axis]);
            b
        },
    )
}
#[test]
fn open_half_sphere_mirrors_closed_with_sphere_volume() {
    let ball = run("ball = solid.sphere@1(radius=0.5, segments=32)\noutput ball");
    let result = run(&format!(
        "{HALF}whole = poly.mirror@1(half, point=[0,0,0], normal=[0,1,0], merge=true)\nsolid = poly.to_solid@1(whole)\noutput solid"
    ));
    let expected = volume(&ball);
    let measured = volume(&result);
    assert!(
        (measured - expected).abs() / expected < 0.01,
        "volume {measured}, expected {expected}"
    );
}
#[test]
fn disjoint_copy_has_expected_extent_and_volume() {
    let result = run(
        "box = solid.box@1(size=[1,1,1])\nfar = xform.move@1(box, by=[2,0,0])\ntris = poly.from_solid@1(far)\nm = poly.mirror@1(tris, point=[0,0,0], normal=[1,0,0], merge=true)\nsolid = poly.to_solid@1(m)\noutput solid",
    );
    let b = bounds(&result, 0);
    assert!(
        (f64::from(b[0]) + 2.5).abs() < 1e-6 && (f64::from(b[1]) - 2.5).abs() < 1e-6,
        "x bounds {b:?}"
    );
    assert!(
        (volume(&result) - 2.0).abs() < 1e-6,
        "volume {}",
        volume(&result)
    );
}
#[test]
fn tilted_plane_places_copy_at_reflected_location() {
    let result = run(
        "box = solid.box@1(size=[1,1,1])\nfar = xform.move@1(box, by=[2,0,0])\ntris = poly.from_solid@1(far)\nm = poly.mirror@1(tris, point=[0,0,0], normal=[1,1,0], merge=true)\nsolid = poly.to_solid@1(m)\noutput solid",
    );
    assert!(
        (volume(&result) - 2.0).abs() < 1e-6,
        "volume {}",
        volume(&result)
    );
    let b = bounds(&result, 1);
    assert!((f64::from(b[0]) + 2.5).abs() < 1e-6, "y bounds {b:?}");
}
#[test]
fn merge_false_keeps_the_seam_vertices_apart() {
    // Converting to a solid fuses coincident points by itself, so closedness cannot tell the two apart; the
    // mirrored mesh's own point count can: with merge=false the half's seam points and their copies stay separate.
    let points = |merge: &str| {
        run(&format!(
            "{HALF}whole = poly.mirror@1(half, point=[0,0,0], normal=[0,1,0], merge={merge})\noutput whole"
        ))
        .parts[0]
            .positions
            .len()
    };
    let (merged, apart) = (points("true"), points("false"));
    assert!(
        apart > merged,
        "merge=false has {apart} points, merge=true {merged}"
    );
    // The half sphere's seam is a ring of 32 points: merging welds each with its mirror image.
    assert_eq!(apart - merged, 32, "seam points welded: {}", apart - merged);
}
#[test]
fn zero_normal_refuses_with_normal_suggestion() {
    let (code,suggestion)=evaluate("box = solid.box@1(size=[1,1,1])\ntris = poly.from_solid@1(box)\nm = poly.mirror@1(tris, normal=[0,0,0])\noutput m").unwrap_err();
    assert_eq!(code, "poly.mirror_normal_zero");
    assert!(
        suggestion.as_ref().is_some_and(|s| s.contains("normal")),
        "suggestion {suggestion:?}"
    );
}
#[test]
fn repeated_evaluations_have_bit_identical_positions() {
    let text = format!(
        "{HALF}whole = poly.mirror@1(half, point=[0,0,0], normal=[0,1,0], merge=true)\nsolid = poly.to_solid@1(whole)\noutput solid"
    );
    let a = run(&text);
    let b = run(&text);
    assert_eq!(a.parts[0].positions, b.parts[0].positions);
}
