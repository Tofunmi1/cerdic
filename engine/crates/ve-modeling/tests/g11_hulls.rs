//! G11: `solid.hull` and `solid.collision_boxes` — convex hulls and fitted collision shapes built on
//! `ve-measure`.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "fixtures are known to be valid and failures report their diagnostic"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(text: &str) -> Result<Evaluation, (String, Option<String>)> {
    let batch = parse_text(&builtin_registry(), text).unwrap();
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g11_hulls").unwrap(),
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

fn bounds(positions: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut low = [f32::INFINITY; 3];
    let mut high = [f32::NEG_INFINITY; 3];
    for point in positions {
        for axis in 0..3 {
            low[axis] = low[axis].min(point[axis]);
            high[axis] = high[axis].max(point[axis]);
        }
    }
    (low, high)
}

fn bbox_volume(positions: &[[f32; 3]]) -> f64 {
    let (low, high) = bounds(positions);
    f64::from(high[0] - low[0]) * f64::from(high[1] - low[1]) * f64::from(high[2] - low[2])
}

const BOX: &str = "box = solid.box@1(size=[1, 2, 3])\n";

#[test]
fn one_hull_of_a_box_is_that_box() {
    let original = run(&format!("{BOX}output box"));
    let hulled = run(&format!("{BOX}h = solid.hull@1(box)\noutput h"));
    let hull = &hulled.parts[0];
    assert!((hull.volume - 6.0).abs() < 1.0e-6, "volume {}", hull.volume);
    println!("box hull volume {}", hull.volume);
    let (low, high) = bounds(&hull.positions);
    let (want_low, want_high) = bounds(&original.parts[0].positions);
    for axis in 0..3 {
        assert!(
            (low[axis] - want_low[axis]).abs() < 1.0e-6
                && (high[axis] - want_high[axis]).abs() < 1.0e-6,
            "axis {axis}: [{}, {}] vs [{}, {}]",
            low[axis],
            high[axis],
            want_low[axis],
            want_high[axis]
        );
    }
}

#[test]
fn two_hull_of_an_l_is_between_the_l_and_its_box() {
    let l = "a = solid.box@1(size=[2, 1, 1])\n\
         b = solid.box@1(size=[1, 2, 1])\n\
         b2 = xform.move@1(b, by=[-0.5, 0.5, 0])\n\
         joined = solid.union@1(a, b2)\n";
    let l_volume = run(&format!("{l}output joined")).parts[0].volume;
    let box_volume = bbox_volume(&run(&format!("{l}output joined")).parts[0].positions);
    let hull_volume = run(&format!("{l}h = solid.hull@1(joined)\noutput h")).parts[0].volume;
    assert!(
        hull_volume > l_volume,
        "hull {hull_volume} not above L {l_volume}"
    );
    assert!(
        hull_volume <= box_volume + 1.0e-6,
        "hull {hull_volume} above bounding box {box_volume}"
    );
    println!("L volume {l_volume}, hull volume {hull_volume}, bounding box volume {box_volume}");
}

#[test]
fn three_hull_of_a_sphere_keeps_its_volume() {
    let sphere = run("ball = solid.sphere@1(radius=0.5, segments=32)\noutput ball").parts[0].volume;
    let hull = run("ball = solid.sphere@1(radius=0.5, segments=32)\n\
         h = solid.hull@1(ball, max_vertices=2000)\noutput h")
    .parts[0]
        .volume;
    assert!(
        (hull - sphere).abs() / sphere < 0.005,
        "sphere {sphere} hull {hull}"
    );
    println!("sphere volume {sphere}, hull volume {hull}");
}

#[test]
fn four_hull_of_a_hull_is_idempotent() {
    let once = run(&format!("{BOX}h = solid.hull@1(box)\noutput h")).parts[0].volume;
    let twice = run(&format!(
        "{BOX}h = solid.hull@1(box)\nh2 = solid.hull@1(h)\noutput h2"
    ))
    .parts[0]
        .volume;
    assert!((once - twice).abs() < 1.0e-9, "{once} vs {twice}");
    println!("hull volume {once}, hull of hull volume {twice}");
}

#[test]
fn five_box_shape_of_a_turned_box_is_axis_aligned_and_larger() {
    let result = run(&format!(
        "{BOX}r = xform.rotate@1(box, angles=[0, 30, 0])\n\
         c = solid.collision_boxes@1(r, shape=\"box\")\noutput c"
    ));
    let part = &result.parts[0];
    assert!(part.volume > 6.0, "volume {}", part.volume);
    assert_eq!(part.indices.len(), 36, "a box has 12 triangles");
    let box_volume = bbox_volume(&part.positions);
    assert!(
        (part.volume - box_volume).abs() < 1.0e-4,
        "axis-aligned box volume {} vs its bounding box {box_volume}",
        part.volume
    );
    println!(
        "turned box bounding volume {box_volume}, box shape volume {}",
        part.volume
    );
}

#[test]
fn six_oriented_box_shape_recovers_the_turned_box() {
    let volume = run(&format!(
        "{BOX}r = xform.rotate@1(box, angles=[0, 30, 0])\n\
         c = solid.collision_boxes@1(r, shape=\"oriented_box\")\noutput c"
    ))
    .parts[0]
        .volume;
    assert!(
        (volume - 6.0).abs() / 6.0 < 0.01,
        "oriented box volume {volume}"
    );
    println!("oriented box volume {volume}");
}

#[test]
fn seven_best_shape_of_a_thin_cylinder_is_within_half_again() {
    let cylinder = "cyl = solid.cylinder@1(radius=0.1, height=2, segments=24)\n";
    let original = run(&format!("{cylinder}output cyl")).parts[0].volume;
    let result = run(&format!(
        "{cylinder}c = solid.collision_boxes@1(cyl, shape=\"best\")\noutput c"
    ));
    let ratio = result.parts[0].volume / original;
    println!("thin cylinder collision shape volume ratio {ratio}");
    assert!(ratio <= 1.5, "volume ratio {ratio}");
    assert!(
        result
            .warnings
            .iter()
            .any(|warning| warning.message.contains("volume ratio")
                && warning.message.contains("capsule")),
        "warnings {:?}",
        result.warnings
    );
}

#[test]
fn eight_a_solid_with_more_points_than_max_vertices_is_refused() {
    let (code, suggestion) = evaluate(
        "ball = solid.sphere@1(radius=0.5, segments=64)\n\
         h = solid.hull@1(ball)\noutput h",
    )
    .unwrap_err();
    assert_eq!(code, "solid.hull_too_many_points");
    let suggestion = suggestion.unwrap_or_default();
    assert!(
        suggestion.contains("max_vertices") || suggestion.to_lowercase().contains("decimate"),
        "suggestion {suggestion:?}"
    );
    println!("sphere(64) refused {code}: {suggestion}");
    let (code, suggestion) = evaluate(&format!(
        "{BOX}h = solid.hull@1(box, max_vertices=4)\noutput h"
    ))
    .unwrap_err();
    assert_eq!(code, "solid.hull_too_many_points");
    let suggestion = suggestion.unwrap_or_default();
    assert!(
        suggestion.contains("max_vertices") || suggestion.to_lowercase().contains("decimate"),
        "suggestion {suggestion:?}"
    );
}

#[test]
fn nine_two_hulls_have_bit_identical_positions() {
    let text = "ball = solid.sphere@1(radius=0.5, segments=16)\n\
                h = solid.hull@1(ball, max_vertices=2000)\noutput h";
    let first = run(text).parts[0].positions.clone();
    let second = run(text).parts[0].positions.clone();
    let bits = |positions: &[[f32; 3]]| -> Vec<u32> {
        positions
            .iter()
            .flatten()
            .map(|value| value.to_bits())
            .collect()
    };
    assert_eq!(bits(&first), bits(&second));
    println!(
        "determinism: {} hull vertices compared bitwise",
        first.len()
    );
}
