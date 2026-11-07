//! `curve.fit`: sketched points fitted with smooth Béziers. Each result is read back through `op('curve.fit@1', ...)` inside a `poly.script`, which
//! returns the sampled curve as data and reports a number as the x extent of a tiny mesh. Written before the body; do not weaken.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "fixture setup unwraps data that is known to be valid"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, (String, String)> {
    let commands = parse_text(&builtin_registry(), source)
        .unwrap_or_else(|e| panic!("{e:?}"))
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_curve_fit").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message.clone()))
}

/// Runs `body` (JavaScript that ends in `out(number)`) with `fit` bound to the fitted curve for `params`, and reads the number back.
fn measure(params: &str, body: &str) -> Result<f32, (String, String)> {
    let source = format!(
        "s = poly.script@1(code=\"const out = (v) => {{ const m = new Mesh(); m.vertex(0,0,0); m.vertex(v,0,0); m.vertex(0,1,0); m.face(0,1,2); return m; }}; const fit = op('curve.fit@1', {params}); const near = (p, q) => Math.hypot(p[0]-q[0], p[1]-q[1], p[2]-q[2]); {body}\")\noutput o = s\n"
    );
    Ok(evaluate(&source)?.parts[0]
        .positions
        .iter()
        .map(|p| p[0])
        .fold(f32::NEG_INFINITY, f32::max))
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

/// JavaScript defining `arc`: points on a quarter circle of radius 1 from (1, 0) to (0, 1).
const ARC: &str = "const arc = Array.from({length: 20}, (_, i) => { const a = i / 19 * Math.PI / 2; return [Math.cos(a), Math.sin(a), 0]; });";

fn with_points(js_points: &str, rest: &str) -> String {
    format!("{{points: {js_points}, {rest}}}")
}

#[test]
fn a_fitted_arc_stays_on_the_circle_and_keeps_its_ends() {
    // `arc` has to exist before the op call, so it is defined by a wrapper that builds the param object.
    let source = format!(
        "s = poly.script@1(code=\"{ARC} const fit = op('curve.fit@1', {}); const out = (v) => {{ const m = new Mesh(); m.vertex(0,0,0); m.vertex(v,0,0); m.vertex(0,1,0); m.face(0,1,2); return m; }}; const radial = Math.max(...fit.points.map(p => Math.abs(Math.hypot(p[0], p[1]) - 1))); out(radial * 1000 + 1);\")\noutput o = s\n",
        with_points("arc", "tolerance: 0.01")
    );
    let error = evaluate(&source).unwrap().parts[0]
        .positions
        .iter()
        .map(|p| p[0])
        .fold(f32::NEG_INFINITY, f32::max)
        - 1.0;
    assert!(
        error < 10.0,
        "no sample strays more than 0.01 from the circle: {error} thousandths"
    );
}

#[test]
fn the_ends_of_an_open_fit_are_the_first_and_last_points() {
    let source = format!(
        "s = poly.script@1(code=\"{ARC} const fit = op('curve.fit@1', {}); const first = fit.points[0], last = fit.points[fit.points.length - 1]; const err = Math.hypot(first[0] - 1, first[1]) + Math.hypot(last[0], last[1] - 1); const m = new Mesh(); m.vertex(0,0,0); m.vertex(err * 1e6 + 1,0,0); m.vertex(0,1,0); m.face(0,1,2); m;\")\noutput o = s\n",
        with_points("arc", "tolerance: 0.01")
    );
    let result = evaluate(&source).unwrap().parts[0]
        .positions
        .iter()
        .map(|p| p[0])
        .fold(f32::NEG_INFINITY, f32::max)
        - 1.0;
    assert!(
        result < 1.0,
        "the curve starts at (1, 0) and ends at (0, 1): {result} millionths off"
    );
}

#[test]
fn points_on_a_line_give_one_straight_piece() {
    let params =
        "{points: [[0,0,0],[1,1,0],[2,2,0],[3,3,0],[4,4,0],[5,5,0]], tolerance: 0.01, steps: 12}";
    let count = measure(params, "out(fit.points.length);").unwrap();
    let off = measure(
        params,
        "out(Math.max(...fit.points.map(p => Math.abs(p[0] - p[1]) + Math.abs(p[2]))) * 1e6 + 1);",
    )
    .unwrap()
        - 1.0;
    assert!(
        near(count, 13.0, 0.01),
        "one Bézier sampled 12 times is 13 points: {count}"
    );
    assert!(
        off < 1.0,
        "every sample is on the line: {off} millionths off"
    );
}

#[test]
fn a_corner_stays_a_corner_without_overshoot() {
    let params = "{points: [[0,0,0],[1,0,0],[2,0,0],[2,1,0],[2,2,0]], tolerance: 0.01}";
    let at_corner = measure(
        params,
        "out(Math.min(...fit.points.map(p => near(p, [2,0,0]))) * 1e6 + 1);",
    )
    .unwrap()
        - 1.0;
    let beyond = measure(
        params,
        "out(Math.max(...fit.points.map(p => Math.max(p[0] - 2, -p[1], 0))) * 1e6 + 1);",
    )
    .unwrap()
        - 1.0;
    assert!(
        at_corner < 1.0,
        "a sample sits exactly on the corner: {at_corner} millionths away"
    );
    assert!(
        beyond < 1.0,
        "and nothing swings past it: {beyond} millionths"
    );
}

#[test]
fn a_corner_angle_of_180_rounds_the_corner_off() {
    let params = "{points: [[0,0,0],[1,0,0],[2,0,0],[2,1,0],[2,2,0]], tolerance: 0.2, corner_angle: 3.14159}";
    let gap = measure(
        params,
        "out(Math.min(...fit.points.map(p => near(p, [2,0,0]))) * 1000 + 1);",
    )
    .unwrap()
        - 1.0;
    assert!(
        gap > 20.0,
        "no corner is kept, so the curve passes more than 2 cm from it: {gap} thousandths"
    );
}

#[test]
fn a_noisy_loop_is_smoothed_and_stays_within_the_tolerance_of_every_point() {
    let noisy = "const pts = Array.from({length: 24}, (_, i) => { const a = i / 24 * 2 * Math.PI; const n = 0.01 * Math.sin(i * 12.9898) ; return [(1 + n) * Math.cos(a), (1 + n) * Math.sin(a), 0]; });";
    let source = format!(
        "s = poly.script@1(code=\"{noisy} const fit = op('curve.fit@1', {}); const seg = (p, a, b) => {{ const d = [b[0]-a[0], b[1]-a[1], b[2]-a[2]]; const l2 = d[0]*d[0] + d[1]*d[1] + d[2]*d[2]; const t = l2 > 0 ? Math.max(0, Math.min(1, ((p[0]-a[0])*d[0] + (p[1]-a[1])*d[1] + (p[2]-a[2])*d[2]) / l2)) : 0; return Math.hypot(p[0]-a[0]-t*d[0], p[1]-a[1]-t*d[1], p[2]-a[2]-t*d[2]); }}; const n = fit.points.length; const worst = Math.max(...pts.map(p => Math.min(...fit.points.map((a, i) => seg(p, a, fit.points[(i + 1) % n]))))); const m = new Mesh(); m.vertex(0,0,0); m.vertex(worst * 1000 + 1,0,0); m.vertex(0,1,0); m.face(0,1,2); m;\")\noutput o = s\n",
        with_points("pts", "tolerance: 0.04, closed: true, steps: 24")
    );
    let worst = evaluate(&source).unwrap().parts[0]
        .positions
        .iter()
        .map(|p| p[0])
        .fold(f32::NEG_INFINITY, f32::max)
        - 1.0;
    assert!(
        worst < 45.0,
        "every input point is within about the tolerance of the curve: {worst} thousandths"
    );
}

#[test]
fn a_closed_fit_is_a_closed_curve_that_does_not_repeat_its_first_point() {
    let ring = "const ring = Array.from({length: 16}, (_, i) => [Math.cos(i / 16 * 2 * Math.PI), Math.sin(i / 16 * 2 * Math.PI), 0]);";
    let source = format!(
        "s = poly.script@1(code=\"{ring} const fit = op('curve.fit@1', {}); const first = fit.points[0], last = fit.points[fit.points.length - 1]; const gap = Math.hypot(first[0]-last[0], first[1]-last[1]); const radial = Math.max(...fit.points.map(p => Math.abs(Math.hypot(p[0], p[1]) - 1))); const m = new Mesh(); m.vertex(0,0,0); m.vertex((fit.closed ? 1000 : 0) + (gap > 1e-6 ? 100 : 0) + radial * 1000,0,0); m.vertex(0,1,0); m.face(0,1,2); m;\")\noutput o = s\n",
        with_points("ring", "tolerance: 0.01, closed: true")
    );
    let code = evaluate(&source).unwrap().parts[0]
        .positions
        .iter()
        .map(|p| p[0])
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(
        code >= 1100.0,
        "closed, and the last point is not the first: {code}"
    );
    assert!(
        code - 1100.0 < 10.0,
        "and within 0.01 of the circle: {} thousandths",
        code - 1100.0
    );
}

#[test]
fn a_curve_that_has_no_two_distinct_points_is_refused_with_the_fix() {
    let error = measure("{points: [[1,1,1],[1,1,1]]}", "out(1);")
        .err()
        .unwrap();
    assert_eq!(error.0, "script.error");
    assert!(error.1.contains("distinct"), "{}", error.1);
}

#[test]
fn a_fitted_curve_feeds_a_tube() {
    let source = "c = curve.fit@1(points=[[0,0,0],[0.5,0.4,0],[1,0.5,0],[1.5,0.3,0],[2,0,0]], tolerance=0.02)\ntube = curve.tube@1(c, radius=0.05, sides=8)\noutput o = tube\n";
    let result = evaluate(source).unwrap();
    let high = result.parts[0]
        .positions
        .iter()
        .map(|p| p[1])
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(high > 0.5, "the tube follows the hump: {high}");
}
