//! G11: plane bisect keeps the requested side, caps it, and stays deterministic.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "fixture failures report their evaluation diagnostic"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

const BOX: &str = "box = solid.box@1(size=[1, 1, 1])\ntris = poly.from_solid@1(box)\nmesh = poly.merge_coplanar@1(tris)\n";

fn evaluate(text: &str) -> Result<Evaluation, (String, Option<String>)> {
    let batch = parse_text(&builtin_registry(), text).unwrap();
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g11_bisect").unwrap(),
        label: None,
        commands: batch.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|error| (error.code.clone(), error.suggestion.clone()))
}

fn run(text: &str) -> Evaluation {
    evaluate(text).unwrap_or_else(|(code, suggestion)| panic!("{code}: {suggestion:?}"))
}

fn box_cut(keep: &str, point: &str, normal: &str, fill: bool) -> String {
    format!(
        "{BOX}cut = poly.bisect@1(mesh, point={point}, normal={normal}, keep=\"{keep}\", fill={fill})\nsolid = poly.to_solid@1(cut)\noutput solid"
    )
}

fn total_volume(result: &Evaluation) -> f64 {
    result.parts.iter().map(|part| part.volume).sum()
}

fn y_bounds(result: &Evaluation) -> [f32; 2] {
    result.parts.iter().flat_map(|part| &part.positions).fold(
        [f32::INFINITY, f32::NEG_INFINITY],
        |mut bounds, point| {
            bounds[0] = bounds[0].min(point[1]);
            bounds[1] = bounds[1].max(point[1]);
            bounds
        },
    )
}

#[test]
fn below_is_capped_with_volume_point_seven() {
    let result = run(&box_cut("below", "[0,0.2,0]", "[0,1,0]", true));
    assert!(
        (total_volume(&result) - 0.7).abs() < 1e-6,
        "volume {}",
        total_volume(&result)
    );
    assert!(
        (f64::from(y_bounds(&result)[1]) - 0.2).abs() < 1e-6,
        "bounds {:?}",
        y_bounds(&result)
    );
}

#[test]
fn above_is_capped_with_volume_point_three() {
    let result = run(&box_cut("above", "[0,0.2,0]", "[0,1,0]", true));
    assert!(
        (total_volume(&result) - 0.3).abs() < 1e-6,
        "volume {}",
        total_volume(&result)
    );
    assert!(
        (f64::from(y_bounds(&result)[0]) - 0.2).abs() < 1e-6,
        "bounds {:?}",
        y_bounds(&result)
    );
}

#[test]
fn both_sides_preserve_volume_and_cut_vertices() {
    let result = run(&box_cut("both", "[0,0.2,0]", "[0,1,0]", true));
    let uncut = run(&format!("{BOX}solid = poly.to_solid@1(mesh)\noutput solid"));
    assert!(
        (total_volume(&result) - 1.0).abs() < 1e-6,
        "volume {}",
        total_volume(&result)
    );
    assert!(result.parts[0].indices.len() / 3 > uncut.parts[0].indices.len() / 3);
}

#[test]
fn tilted_plane_halves_the_box() {
    let result = run(&box_cut("below", "[0,0,0]", "[1,1,0]", true));
    assert!(
        (total_volume(&result) - 0.5).abs() < 1e-6,
        "volume {}",
        total_volume(&result)
    );
}

#[test]
fn distant_plane_keeps_the_box() {
    let result = run(&box_cut("below", "[0,5,0]", "[0,1,0]", true));
    assert!(
        (total_volume(&result) - 1.0).abs() < 1e-6,
        "volume {}",
        total_volume(&result)
    );
}

#[test]
fn removing_everything_is_a_typed_refusal_with_suggestion() {
    let (code, suggestion) = evaluate(&box_cut("above", "[0,5,0]", "[0,1,0]", true)).unwrap_err();
    assert_eq!(code, "poly.bisect_empty");
    assert!(suggestion.as_ref().is_some_and(|text| !text.is_empty()));
}

#[test]
fn zero_normal_is_refused_with_normal_suggestion() {
    let (code, suggestion) = evaluate(&box_cut("below", "[0,0,0]", "[0,0,0]", true)).unwrap_err();
    assert_eq!(code, "poly.bisect_normal_zero");
    assert!(
        suggestion
            .as_ref()
            .is_some_and(|text| text.contains("normal"))
    );
}

#[test]
fn identical_programs_have_bit_identical_positions() {
    let text = box_cut("below", "[0,0.2,0]", "[0,1,0]", true);
    let first = run(&text);
    let second = run(&text);
    assert_eq!(first.parts[0].positions, second.parts[0].positions);
}

#[test]
fn bisected_uv_sphere_keeps_half_its_volume() {
    let fixture = "sphere = solid.sphere@1(radius=0.5, segments=32)\ntris = poly.from_solid@1(sphere)\nmesh = poly.merge_coplanar@1(tris)\n";
    let original = run(&format!(
        "{fixture}solid = poly.to_solid@1(mesh)\noutput solid"
    ));
    let result = run(&format!(
        "{fixture}cut = poly.bisect@1(mesh, point=[0,0,0], normal=[0,1,0], keep=\"below\")\nsolid = poly.to_solid@1(cut)\noutput solid"
    ));
    let expected = total_volume(&original) / 2.0;
    let measured = total_volume(&result);
    assert!(
        (measured - expected).abs() / expected < 0.01,
        "volume {measured}, expected {expected}"
    );
}
