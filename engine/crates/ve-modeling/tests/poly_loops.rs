//! The edge-loop tools: space evenly, relax, circularize. Each is run on the top ring of a small tube picked with `poly.select`, and a
//! `poly.script` reads the ring back from `input` and reports how uneven it is. Written before the body; do not weaken.
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
        transaction: TransactionId::parse("t_poly_loops").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message.clone()))
}

/// A tube of eight quads: a bottom ring of radius 1 at y = 0 and a top ring at y = 1 whose points are `ring` (JavaScript giving
/// `[x, z]` for point `i`, 0 to 7).
fn tube(ring: &str) -> String {
    format!(
        "tube = poly.script@1(code=\"const m = new Mesh(); const ring = (i) => {ring}; for (let i = 0; i < 8; i++) {{ const p = ring(i); m.vertex(p[0], 1, p[1]); }} for (let i = 0; i < 8; i++) m.vertex(Math.cos(i * Math.PI / 4), 0, Math.sin(i * Math.PI / 4)); for (let i = 0; i < 8; i++) m.face(i, (i + 1) % 8, 8 + (i + 1) % 8, 8 + i); m\")\ntop = poly.select@1(tube, element=\"edge\", by=\"height\", axis=\"y\", height=1, tolerance=0.01)"
    )
}

/// JavaScript over `input`: how uneven the top ring is, times 1000 (`gap` the spread of the angles between neighbours, `radius` the spread of the distances from the axis).
fn spread(kind: &str) -> String {
    let top = "const top = input.points.filter(p => Math.abs(p[1] - 1) < 0.001);";
    match kind {
        "gap" => format!(
            "(() => {{ {top} const a = top.map(p => Math.atan2(p[2], p[0])).sort((x, y) => x - y); const g = a.map((v, i) => (i + 1 < a.length ? a[i + 1] - v : a[0] + 2 * Math.PI - v)); return (Math.max(...g) - Math.min(...g)) * 1000; }})()"
        ),
        _ => format!(
            "(() => {{ {top} const r = top.map(p => Math.hypot(p[0], p[2])); return (Math.max(...r) - Math.min(...r)) * 1000; }})()"
        ),
    }
}

fn read(setup: &str, node: &str, value: &str) -> f32 {
    let source = format!(
        "{setup}\nm = poly.script@1(in={node}, code=\"const o = new Mesh(); o.vertex(0,0,0); o.vertex({value},0,0); o.vertex(0,1,0); o.face(0,1,2); o\")\noutput o = m\n"
    );
    let result = evaluate(&source).unwrap();
    result.parts[0]
        .positions
        .iter()
        .map(|p| p[0])
        .fold(f32::NEG_INFINITY, f32::max)
}

/// Unevenly spaced angles on a true circle.
const UNEVEN_ANGLES: &str = "(() => { const a = i * Math.PI / 4 + [0, 0.3, -0.2, 0.25, 0, -0.3, 0.2, 0][i]; return [Math.cos(a), Math.sin(a)]; })()";
/// Evenly spaced angles on an ellipse (1.5 by 0.75).
const ELLIPSE: &str = "[1.5 * Math.cos(i * Math.PI / 4), 0.75 * Math.sin(i * Math.PI / 4)]";
/// Evenly spaced angles with a zigzag radius.
const ZIGZAG: &str = "(() => { const r = i % 2 === 0 ? 1 : 1.4; return [r * Math.cos(i * Math.PI / 4), r * Math.sin(i * Math.PI / 4)]; })()";

#[test]
fn space_loops_evens_the_gaps_between_a_rings_vertices() {
    let setup = format!(
        "{}\nsp = poly.space_loops@1(tube, top, factor=1)",
        tube(UNEVEN_ANGLES)
    );
    let before = read(&setup, "tube", &spread("gap"));
    let after = read(&setup, "sp", &spread("gap"));
    assert!(
        before > 400.0,
        "the gaps start uneven (0.3 rad apart or more): {before}"
    );
    assert!(after < 60.0, "and end close to even: {after}");
}

#[test]
fn a_factor_of_zero_leaves_the_ring_as_it_was() {
    let setup = format!(
        "{}\nsp = poly.space_loops@1(tube, top, factor=0)",
        tube(UNEVEN_ANGLES)
    );
    let before = read(&setup, "tube", &spread("gap"));
    let after = read(&setup, "sp", &spread("gap"));
    assert!((before - after).abs() < 2.0, "{before} then {after}");
}

#[test]
fn relax_loops_smooths_a_zigzag_ring() {
    let setup = format!(
        "{}\nrl = poly.relax_loops@1(tube, top, iterations=10)",
        tube(ZIGZAG)
    );
    let before = read(&setup, "tube", &spread("radius"));
    let after = read(&setup, "rl", &spread("radius"));
    assert!(before > 350.0, "the radius swings by 0.4 m: {before}");
    assert!(
        after < before * 0.7,
        "ten passes calm it: {after} from {before}"
    );
}

#[test]
fn circularize_turns_an_ellipse_into_a_circle() {
    let setup = format!(
        "{}\nci = poly.circularize@1(tube, top, factor=1)",
        tube(ELLIPSE)
    );
    let before = read(&setup, "tube", &spread("radius"));
    let after = read(&setup, "ci", &spread("radius"));
    assert!(before > 700.0, "the radii start 0.75 m apart: {before}");
    assert!(after < 30.0, "and end within 3 cm of each other: {after}");
}

#[test]
fn circularize_with_a_radius_uses_it() {
    let setup = format!(
        "{}\nci = poly.circularize@1(tube, top, factor=1, radius=2)",
        tube(ELLIPSE)
    );
    let mean = read(
        &setup,
        "ci",
        "(() => { const top = input.points.filter(p => Math.abs(p[1] - 1) < 0.001); return top.reduce((s, p) => s + Math.hypot(p[0], p[2]), 0) / top.length * 1000; })()",
    );
    assert!(
        (mean - 2000.0).abs() < 60.0,
        "the ring sits at radius 2 m: {mean}"
    );
}

#[test]
fn a_loop_tool_refuses_faces_and_asks_for_edges() {
    let error = evaluate(&format!(
        "{}\nall = poly.select@1(tube, element=\"face\", by=\"all\")\nsp = poly.space_loops@1(tube, all)\noutput o = sp\n",
        tube(ELLIPSE)
    ))
    .err()
    .unwrap();
    assert_eq!(error.0, "poly.selection_kind");
    assert!(error.1.contains("edge"), "{}", error.1);
}
