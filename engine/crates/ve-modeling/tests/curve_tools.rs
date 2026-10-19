//! `curve.spline`, `curve.helix`, `curve.fillet`, `curve.resample`, `curve.reverse`: each is swept with a round profile
//! and the tube's volume is compared with area times length.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(text: &str) -> Result<Evaluation, String> {
    let batch = parse_text(&builtin_registry(), text).map_err(|e| format!("{e:?}"))?;
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_curve_tools").unwrap(),
        label: None,
        commands: batch.commands,
    });
    if let Outcome::Rejected { .. } = outcome {
        return Err(format!("{outcome:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| format!("{}: {}", e.code, e.message))
}

/// Area of the 12-sided profile of radius 0.03.
fn tube_area() -> f64 {
    0.5 * 12.0 * 0.03_f64.powi(2) * (std::f64::consts::TAU / 12.0).sin()
}

fn tube_volume(curve: &str) -> Result<f64, String> {
    let program = format!(
        "{curve}\nring = profile.circle@1(radius=0.03, segments=12)\ntube = solid.sweep@1(path, ring)\noutput tube = tube\n"
    );
    Ok(evaluate(&program)?.parts[0].volume)
}

#[test]
fn a_spline_through_collinear_points_is_the_straight_line() {
    let v = tube_volume("path = curve.spline@1(points=[[0, 0, 0], [0, 1, 0], [0, 3, 0]], steps=6)")
        .unwrap();
    assert!((v - tube_area() * 3.0).abs() < tube_area() * 0.03, "{v}");
}

#[test]
fn a_helix_tube_has_the_volume_of_its_length_times_the_area() {
    let (radius, pitch, turns, per) = (0.5_f64, 0.3_f64, 2.0_f64, 32.0_f64);
    let chord = ((2.0 * radius * (std::f64::consts::PI / per).sin()).powi(2)
        + (pitch / per).powi(2))
    .sqrt();
    let expected = tube_area() * chord * per * turns;
    let v =
        tube_volume("path = curve.helix@1(radius=0.5, pitch=0.3, turns=2, segments_per_turn=32)")
            .unwrap();
    assert!((v - expected).abs() < expected * 0.06, "{v} vs {expected}");
}

#[test]
fn resample_and_reverse_keep_the_shape() {
    let line = "line = curve.polyline@1(points=[[0, 0, 0], [0, 2, 0]])";
    let base = tube_volume(&format!("{line}\npath = curve.resample@1(line, count=11)")).unwrap();
    assert!(
        (base - tube_area() * 2.0).abs() < tube_area() * 0.03,
        "{base}"
    );
    let reversed = tube_volume(&format!(
        "{line}\nmore = curve.resample@1(line, count=5)\npath = curve.reverse@1(more)"
    ))
    .unwrap();
    assert!(
        (reversed - base).abs() < tube_area() * 0.03,
        "{reversed} vs {base}"
    );
}

#[test]
fn a_fillet_shortens_the_route_and_still_sweeps_cleanly() {
    let corner = "route = curve.polyline@1(points=[[0, 0, 0], [1, 0, 0], [1, 1, 0]])";
    let sharp = tube_volume(&format!(
        "{corner}\npath = curve.resample@1(route, count=40)"
    ))
    .unwrap();
    let round = tube_volume(&format!(
        "{corner}\npath = curve.fillet@1(route, distance=0.3, steps=8)"
    ))
    .unwrap();
    assert!(
        round < sharp && round > sharp * 0.9,
        "cutting the corner shortens the route a little: {round} vs {sharp}"
    );
}

#[test]
fn a_tapered_sweep_is_the_frustum_of_its_scale_and_a_twisted_one_keeps_its_volume() {
    let line = "route = curve.polyline@1(points=[[0, 0, 0], [0, 2, 0]])\npath = curve.resample@1(route, count=41)";
    let taper = |scale: f64| {
        let program = format!(
            "{line}\nring = profile.circle@1(radius=0.03, segments=12)\ntube = solid.sweep@1(path, ring, scale_end={scale})\noutput tube = tube\n"
        );
        evaluate(&program).map(|e| e.parts[0].volume)
    };
    let expected = tube_area() * 2.0 * (1.0 + 0.5 + 0.25) / 3.0;
    let v = taper(0.5).unwrap();
    assert!((v - expected).abs() < expected * 0.01, "{v} vs {expected}");
    assert!(
        taper(2.0).unwrap() > tube_area() * 2.0 * 2.0,
        "growing to double radius makes a fatter tube"
    );
    let twisted = format!(
        "{line}\nbar = profile.rect@1(size=[0.06, 0.06])\nrope = solid.sweep@1(path, bar, twist=3.14159)\noutput rope = rope\n"
    );
    let v = evaluate(&twisted).unwrap().parts[0].volume;
    assert!(
        (v - 0.06 * 0.06 * 2.0).abs() < 0.0072 * 0.04,
        "a twisted bar keeps nearly all of its volume: {v}"
    );
}

#[test]
fn bad_curves_are_refused_with_a_reason() {
    let closed = "loop = curve.circle@1(radius=0.5, segments=16)\nring = profile.circle@1(radius=0.03, segments=12)\nt = solid.sweep@1(loop, ring, scale_end=0.5)\noutput t = t\n";
    assert!(
        evaluate(closed).unwrap_err().contains("open curve"),
        "a closed loop cannot taper"
    );
    assert!(
        evaluate("p = curve.spline@1(points=[[0, 0, 0], [1, 0, 0]], closed=true)\noutput o = p\n")
            .is_err(),
        "closed needs 3"
    );
    assert!(
        tube_volume("path = curve.helix@1(radius=0.5, pitch=0.2, turns=64, segments_per_turn=128)")
            .is_err(),
        "8,193 samples is over the cap"
    );
    let dup =
        tube_volume("path = curve.spline@1(points=[[0, 0, 0], [0, 0, 0], [0, 1, 0]])").unwrap_err();
    assert!(
        dup.contains("degenerate") || dup.contains("repeated"),
        "{dup}"
    );
}
