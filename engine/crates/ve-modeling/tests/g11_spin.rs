//! G11: lathe a polygon strip and preserve selection, seams, and repeatability.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "fixture failures report their evaluation diagnostic"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

const STRIP: &str = "strip = poly.grid@1(width=0.2, height=1.0, columns=1, rows=4)\n";
const SPIN: &str = "spun = poly.spin@1(strip, center=[-1,0,0], axis=[0,1,0], angle=6.283185307179586, steps=64, merge=true)\n";
fn expected(steps: f64) -> f64 {
    0.5 * steps * (std::f64::consts::TAU / steps).sin() * (1.1 * 1.1 - 0.9 * 0.9)
}

fn evaluate(text: &str) -> Result<Evaluation, (String, Option<String>)> {
    let batch = parse_text(&builtin_registry(), text).unwrap();
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g11_spin").unwrap(),
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

fn surface_area(result: &Evaluation) -> f64 {
    result
        .parts
        .iter()
        .map(|part| {
            part.indices
                .chunks_exact(3)
                .map(|tri| {
                    let a = part.positions[tri[0] as usize];
                    let b = part.positions[tri[1] as usize];
                    let c = part.positions[tri[2] as usize];
                    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                    0.5 * f64::from(
                        ((u[1] * v[2] - u[2] * v[1]).powi(2)
                            + (u[2] * v[0] - u[0] * v[2]).powi(2)
                            + (u[0] * v[1] - u[1] * v[0]).powi(2))
                        .sqrt(),
                    )
                })
                .sum::<f64>()
        })
        .sum()
}

#[test]
fn full_turn_is_closed_with_polygonal_ring_volume() {
    run(&format!(
        "{STRIP}{SPIN}tri = poly.triangulate@1(spun)\noutput tri"
    ));
    let result = run(&format!(
        "{STRIP}{SPIN}solid = poly.to_solid@1(spun)\noutput solid"
    ));
    let measured = volume(&result);
    assert!(
        (measured - expected(64.0)).abs() / expected(64.0) < 0.005,
        "volume {measured}, expected {}",
        expected(64.0)
    );
}

#[test]
fn full_turn_has_expected_outer_diameter() {
    let result = run(&format!("{STRIP}{SPIN}output spun"));
    let x = bounds(&result, 0);
    let z = bounds(&result, 2);
    let x_extent = f64::from(x[1] - x[0]);
    let z_extent = f64::from(z[1] - z[0]);
    assert!(
        (x_extent - 2.2).abs() / 2.2 < 0.005,
        "x extent {x_extent}, bounds {x:?}"
    );
    assert!(
        (z_extent - 2.2).abs() / 2.2 < 0.005,
        "z extent {z_extent}, bounds {z:?}"
    );
}

#[test]
fn sixteen_steps_are_coarser_than_sixty_four() {
    let coarse = run(&format!(
        "{STRIP}coarse = poly.spin@1(strip, center=[-1,0,0], axis=[0,1,0], angle=6.283185307179586, steps=16, merge=true)\nsolid = poly.to_solid@1(coarse)\noutput solid"
    ));
    let fine = run(&format!(
        "{STRIP}{SPIN}solid = poly.to_solid@1(spun)\noutput solid"
    ));
    let expected = expected(16.0);
    let measured = volume(&coarse);
    assert!(
        measured < volume(&fine),
        "coarse {measured}, fine {}",
        volume(&fine)
    );
    assert!(
        (measured - expected).abs() / expected < 0.005,
        "volume {measured}, expected {expected}"
    );
}

#[test]
fn half_turn_is_typed_open_surface_refusal() {
    let (code, suggestion) = evaluate(&format!("{STRIP}spun = poly.spin@1(strip, center=[-1,0,0], axis=[0,1,0], angle=3.141592653589793, steps=32, merge=true)\nsolid = poly.to_solid@1(spun)\noutput solid")).unwrap_err();
    assert!(code.starts_with("poly."), "code {code}");
    assert!(
        suggestion
            .as_ref()
            .is_some_and(|s| s.contains("angle") || s.contains("merge")),
        "suggestion {suggestion:?}"
    );
}

#[test]
fn merge_false_refuses_the_unwelded_seam() {
    let (code, suggestion) = evaluate(&format!("{STRIP}spun = poly.spin@1(strip, center=[-1,0,0], axis=[0,1,0], angle=6.283185307179586, steps=64, merge=false)\nsolid = poly.to_solid@1(spun)\noutput solid")).unwrap_err();
    assert!(code.starts_with("poly."), "code {code}");
    assert!(
        suggestion.as_ref().is_some_and(|s| s.contains("merge")),
        "suggestion {suggestion:?}"
    );
}

#[test]
fn zero_axis_is_refused_with_axis_suggestion() {
    let (code, suggestion) = evaluate(&format!(
        "{STRIP}spun = poly.spin@1(strip, axis=[0,0,0])\noutput spun"
    ))
    .unwrap_err();
    assert_eq!(code, "poly.spin_axis_zero");
    assert!(
        suggestion.as_ref().is_some_and(|s| s.contains("axis")),
        "suggestion {suggestion:?}"
    );
}

#[test]
fn identical_full_turns_have_bit_identical_positions() {
    let text = format!("{STRIP}{SPIN}solid = poly.to_solid@1(spun)\noutput solid");
    let first = run(&text);
    let second = run(&text);
    assert_eq!(first.parts[0].positions, second.parts[0].positions);
}

#[test]
fn selected_bottom_edge_revolves_into_open_washer() {
    let selection = "edges = poly.select@1(strip, element=\"edge\", by=\"box\", box_min=[-0.2,-0.01,-0.01], box_max=[0.2,0.01,0.01])\n";
    let spun = "spun = poly.spin@1(strip, edges, center=[-1,0,0], axis=[0,1,0], angle=6.283185307179586, steps=64, merge=true)\n";
    let result = run(&format!(
        "{STRIP}{selection}{spun}tri = poly.triangulate@1(spun)\noutput tri"
    ));
    // Spinning an edge leaves the mesh's own faces where they were (Blender's behaviour), so the strip's 0.2 x 1.0 m stays beside the washer.
    let measured = surface_area(&result) - 0.2;
    assert!(
        (measured - expected(64.0)).abs() / expected(64.0) < 0.005,
        "washer area {measured}, expected {}",
        expected(64.0)
    );
    let strip = run(&format!("{STRIP}output strip"));
    assert!(
        result.parts[0].positions.len() > strip.parts[0].positions.len(),
        "spin points {}, strip points {}",
        result.parts[0].positions.len(),
        strip.parts[0].positions.len()
    );
    let error = evaluate(&format!("{STRIP}{selection}{spun}tri = poly.triangulate@1(spun)\nsolid = poly.to_solid@1(tri)\noutput solid")).unwrap_err();
    assert!(error.0.starts_with("poly."), "code {}", error.0);
}
