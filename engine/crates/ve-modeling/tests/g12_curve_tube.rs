//! `curve.tube@1` through the text language and the evaluator: volumes, the real shape of the mesh, and the refusals.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "fixtures are fixed valid evaluator scenes"
)]
use std::f64::consts::{FRAC_PI_4, TAU};
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn run(text: &str) -> Result<Evaluation, (String, Option<String>)> {
    let batch = parse_text(&builtin_registry(), text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g12_curve_tube").unwrap(),
        label: None,
        commands: batch.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.suggestion.clone()))
}

fn tube(curve: &str, settings: &str) -> Evaluation {
    run(&format!(
        "a = {curve}\nt = curve.tube@1(a{settings})\noutput t"
    ))
    .unwrap_or_else(|(code, _)| panic!("{code}"))
}

fn error(curve: &str, settings: &str) -> (String, String) {
    let (code, suggestion) = run(&format!(
        "a = {curve}\nt = curve.tube@1(a{settings})\noutput t"
    ))
    .unwrap_err();
    (code, suggestion.unwrap_or_default())
}

/// The vertices of the first part, as f64.
fn vertices(result: &Evaluation) -> Vec<[f64; 3]> {
    result.parts[0]
        .positions
        .iter()
        .map(|p| p.map(f64::from))
        .collect()
}

const UP_2: &str = "curve.polyline@1(points=[[0,0,0],[0,2,0]])";

/// The area of a regular `n`-gon of circumradius `r`.
fn polygon_area(n: u32, r: f64) -> f64 {
    f64::from(n) / 2.0 * r * r * (TAU / f64::from(n)).sin()
}

#[test]
fn a_straight_tube_has_the_polygon_volume_and_the_expected_bounding_box() {
    let result = tube(UP_2, ", radius=0.1, sides=32");
    let expected = 2.0 * polygon_area(32, 0.1);
    assert!((result.parts[0].volume - expected).abs() / expected < 1e-6);
    let points = vertices(&result);
    let extent = |axis: usize| {
        let values = points.iter().map(|p| p[axis]);
        (
            values.clone().fold(f64::INFINITY, f64::min),
            values.fold(f64::NEG_INFINITY, f64::max),
        )
    };
    for axis in [0, 2] {
        let (low, high) = extent(axis);
        assert!(
            (low + 0.1).abs() < 1e-6 && (high - 0.1).abs() < 1e-6,
            "axis {axis}: {low} to {high}"
        );
    }
    let (low, high) = extent(1);
    assert!(
        low.abs() < 1e-6 && (high - 2.0).abs() < 1e-6,
        "y: {low} to {high}"
    );
}

#[test]
fn a_taper_is_a_cone_frustum() {
    let result = tube(UP_2, ", radius=0.2, end_radius=0.1, sides=64");
    let (a0, a1) = (polygon_area(64, 0.2), polygon_area(64, 0.1));
    let expected = 2.0 / 3.0 * (a0 + a1 + (a0 * a1).sqrt());
    assert!((result.parts[0].volume - expected).abs() / expected < 1e-5);
}

#[test]
fn radii_stations_set_the_radius_along_the_length() {
    // Stations 0.3, 0.1, 0.1 at the start, middle and end of 4 m: the radius is linear between them, so the rings at
    // y = 0, 1, 2, 3, 4 have radii 0.3, 0.2, 0.1, 0.1, 0.1.
    let curve = "curve.polyline@1(points=[[0,0,0],[0,1,0],[0,2,0],[0,3,0],[0,4,0]])";
    let result = tube(curve, ", radii=\"0.3, 0.1, 0.1\", sides=16");
    let expected = |y: f64| if y <= 2.0 { 0.3 - 0.1 * y } else { 0.1 };
    let mut rings = 0;
    for p in vertices(&result) {
        let radius = p[0].hypot(p[2]);
        if radius < 1e-6 {
            continue; // the centre of an end cap
        }
        rings += 1;
        assert!(
            (radius - expected(p[1])).abs() < 1e-5,
            "radius {radius} at y = {}",
            p[1]
        );
    }
    assert_eq!(rings, 5 * 16);
}

#[test]
fn a_bend_is_mitred_in_the_bend_plane_and_round_out_of_it() {
    let result = tube(
        "curve.polyline@1(points=[[0,0,0],[1,0,0],[1,1,0]])",
        ", radius=0.05, sides=16",
    );
    assert!(result.parts[0].volume > 0.0);
    let corner = [1.0, 0.0, 0.0];
    let bisector = [FRAC_PI_4.cos(), FRAC_PI_4.sin(), 0.0];
    // The miter direction in the bend plane, perpendicular to the bisector.
    let miter = [-FRAC_PI_4.sin(), FRAC_PI_4.cos(), 0.0];
    let ring: Vec<[f64; 3]> = vertices(&result)
        .into_iter()
        .filter(|p| ((p[0] - 1.0).powi(2) + p[1].powi(2) + p[2].powi(2)).sqrt() < 0.2)
        .collect();
    assert_eq!(ring.len(), 16);
    let (mut along, mut out_of_plane) = (0.0_f64, 0.0_f64);
    for p in &ring {
        let offset = [p[0] - corner[0], p[1] - corner[1], p[2] - corner[2]];
        let flat: f64 = offset.iter().zip(bisector).map(|(a, b)| a * b).sum();
        assert!(
            flat.abs() < 1e-6,
            "ring vertex off the bisecting plane by {flat}"
        );
        along = along.max(
            offset
                .iter()
                .zip(miter)
                .map(|(a, b)| a * b)
                .sum::<f64>()
                .abs(),
        );
        out_of_plane = out_of_plane.max(offset[2].abs());
    }
    assert!(
        (along - 0.05 / FRAC_PI_4.cos()).abs() < 1e-5,
        "extent along the bend {along}"
    );
    assert!(
        (out_of_plane - 0.05).abs() < 1e-6,
        "extent out of the bend plane {out_of_plane}"
    );
}

#[test]
fn a_closed_curve_makes_a_ring_tube_with_no_caps() {
    let result = tube(
        "curve.circle@1(radius=1, segments=24)",
        ", radius=0.1, sides=24",
    );
    let expected = TAU * polygon_area(24, 0.1);
    assert!((result.parts[0].volume - expected).abs() / expected < 0.02);
    // 24 rings of 24 vertices and no cap centres.
    assert_eq!(result.parts[0].positions.len(), 24 * 24);
}

#[test]
fn caps_close_the_ends_and_an_open_tube_is_refused() {
    let capped = tube(UP_2, ", sides=8, caps=true");
    assert_eq!(capped.parts[0].indices.len() / 3, 8 * 2 + 8 * 2);
    // Without caps the ends are open, which is not a solid: refused with the fix. A closed curve has no ends to cap.
    let (code, suggestion) = error(UP_2, ", sides=8, caps=false");
    assert_eq!(code, "geometry.unsupported_topology");
    assert!(suggestion.contains("caps=true"), "{suggestion}");
    let ring = tube(
        "curve.circle@1(radius=1, segments=24)",
        ", radius=0.1, sides=8, caps=false",
    );
    assert!(ring.parts[0].volume > 0.0);
}

#[test]
fn refusals_name_the_fix() {
    let (code, _) = error(UP_2, ", radius=0");
    assert_eq!(code, "geometry.degenerate_profile");
    let (code, suggestion) = error(
        "curve.polyline@1(points=[[0,0,0],[1,0,0],[0.01,0.05,0]])",
        "",
    );
    assert_eq!(code, "geometry.unsupported_topology");
    assert!(suggestion.contains("curve.fillet"), "{suggestion}");
    assert_eq!(error(UP_2, ", radii=\"0.1\"").0, "param.range");
    assert_eq!(error(UP_2, ", radii=\"a, b\"").0, "param.type");
    assert_eq!(error(UP_2, ", radii=\"0.1, 500\"").0, "param.range");
    let (code, suggestion) = error("curve.polyline@1(points=[[0,0,0],[0,0,0],[0,1,0]])", "");
    assert_eq!(code, "geometry.degenerate_curve");
    assert!(suggestion.contains("repeated"), "{suggestion}");
}

#[test]
fn a_curve_with_a_single_point_is_refused_before_it_reaches_the_tube() {
    // `curve.polyline` itself needs at least two points, so a one-point curve never exists to be tubed.
    let refused = parse_text(
        &builtin_registry(),
        "a = curve.polyline@1(points=[[0,0,0]])\nt = curve.tube@1(a)\noutput t",
    )
    .unwrap_err();
    assert_eq!(refused[0].code, "param.range");
    assert_eq!(refused[0].param.as_deref(), Some("points"));
    assert_eq!(refused[0].limit, Some(2.0));
}

#[test]
fn the_registry_describes_the_tube_for_the_operator_index() {
    let registry = builtin_registry();
    let spec = registry
        .iter()
        .find(|op| op.id.to_string() == "curve.tube@1")
        .unwrap();
    assert!(
        spec.doc
            .contains("trunks, branches, vines, pipes, roots, and cables")
    );
}
