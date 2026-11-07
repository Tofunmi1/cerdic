//! `poly.remesh_quads`: organic meshes become even all-quad meshes; shapes the remesher cannot keep are refused with the reason. Each result is
//! read back by a `poly.script` over `input`. Written before the body; do not weaken.
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
        transaction: TransactionId::parse("t_poly_remesh").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message.clone()))
}

fn read(setup: &str, node: &str, value: &str) -> f32 {
    let source = format!(
        "{setup}\nm = poly.script@1(in={node}, code=\"const o = new Mesh(); o.vertex(0,0,0); o.vertex({value},0,0); o.vertex(0,1,0); o.face(0,1,2); o\")\noutput o = m\n"
    );
    evaluate(&source).unwrap().parts[0]
        .positions
        .iter()
        .map(|p| p[0])
        .fold(f32::NEG_INFINITY, f32::max)
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

/// A creature: a torso, a head and two arms smooth-unioned and meshed from a field (uneven triangles).
const CREATURE: &str = "torso = field.capsule@1(radius=0.22, half_length=0.25)\nhead = field.sphere@1(radius=0.17)\nhead_up = field.move@1(head, by=[0, 0.46, 0.04])\narm = field.capsule@1(radius=0.07, half_length=0.22)\narm_l = field.move@1(arm, by=[0.34, 0.1, 0])\narm_r = field.move@1(arm, by=[-0.34, 0.1, 0])\nupper = field.smooth_union@1(torso, head_up, radius=0.08)\nleft = field.smooth_union@1(upper, arm_l, radius=0.08)\nbody = field.smooth_union@1(left, arm_r, radius=0.08)\nmesh = field.to_solid@1(body, min=[-0.7, -0.6, -0.4], max=[0.7, 0.8, 0.4], edge_length=0.05)\npoly = poly.from_solid@1(mesh)";

/// JavaScript over `input`: 1 when every face is a quad.
const ALL_QUADS: &str = "(input.faces.every(f => f.length === 4) ? 1 : 0)";
/// JavaScript over `input`: 1 when every edge is shared by exactly two faces (a closed surface).
const CLOSED: &str = "(() => { const e = new Map(); input.faces.forEach(f => f.forEach((a, i) => { const b = f[(i + 1) % f.length]; const k = Math.min(a, b) + ':' + Math.max(a, b); e.set(k, (e.get(k) || 0) + 1); })); return [...e.values()].every(n => n === 2) ? 1 : 0; })()";
/// JavaScript over `input`: the enclosed volume times 1000, from the faces fanned into triangles.
const VOLUME: &str = "(() => { let v = 0; input.faces.forEach(f => { for (let k = 1; k + 1 < f.length; k++) { const a = input.points[f[0]], b = input.points[f[k]], c = input.points[f[k + 1]]; v += a[0]*(b[1]*c[2]-b[2]*c[1]) - a[1]*(b[0]*c[2]-b[2]*c[0]) + a[2]*(b[0]*c[1]-b[1]*c[0]); } }); return v / 6 * 1000 + 100000; })()";

#[test]
fn a_creature_becomes_about_the_requested_number_of_quads_all_of_them_quads() {
    let setup = format!("{CREATURE}\nq = poly.remesh_quads@1(poly, faces=800)");
    let faces = read(&setup, "q", "input.faces.length");
    assert!(
        faces > 650.0 && faces < 1000.0,
        "asked for 800 quads, got {faces}"
    );
    assert_eq!(read(&setup, "q", ALL_QUADS), 1.0, "every face is a quad");
}

#[test]
fn the_result_is_a_closed_surface() {
    let setup = format!("{CREATURE}\nq = poly.remesh_quads@1(poly, faces=800)");
    assert_eq!(read(&setup, "q", CLOSED), 1.0, "every edge joins two faces");
}

#[test]
fn the_same_seed_gives_the_same_mesh_and_another_seed_a_different_one() {
    let sum = "input.points.reduce((s, p) => s + p[0] * 3 + p[1] * 5 + p[2] * 7, 0) + 1000";
    let a = read(
        &format!("{CREATURE}\nq = poly.remesh_quads@1(poly, faces=500, seed=3)"),
        "q",
        sum,
    );
    let b = read(
        &format!("{CREATURE}\nq = poly.remesh_quads@1(poly, faces=500, seed=3)"),
        "q",
        sum,
    );
    let c = read(
        &format!("{CREATURE}\nq = poly.remesh_quads@1(poly, faces=500, seed=9)"),
        "q",
        sum,
    );
    assert_eq!(a, b);
    assert!(
        (a - c).abs() > 1e-4,
        "another seed gives another layout: {a} and {c}"
    );
}

#[test]
fn the_result_keeps_the_size_and_the_outside_of_the_shape() {
    let setup = "s = solid.sphere@1(radius=1, segments=32)\np = poly.from_solid@1(s)\nq = poly.remesh_quads@1(p, faces=400)";
    let volume = (read(setup, "q", VOLUME) - 100_000.0) / 1000.0;
    let sphere = 4.0 / 3.0 * std::f32::consts::PI;
    assert!(
        volume > 0.0,
        "facing outward gives a positive volume: {volume}"
    );
    assert!(
        near(volume, sphere, sphere * 0.06),
        "a unit sphere's volume {sphere}, remeshed {volume}"
    );
}

#[test]
fn a_hard_surface_shape_is_refused_with_the_reason_not_returned_with_holes() {
    let error = evaluate(
        "cube = solid.box@1(size=[1, 1, 1])\nball = solid.sphere@1(radius=0.62)\ncut = solid.difference@1(cube, ball)\np = poly.from_solid@1(cut)\nq = poly.remesh_quads@1(p, faces=500)\noutput o = q\n",
    )
    .err()
    .unwrap();
    assert_eq!(error.0, "poly.remesh_unfaithful");
    assert!(error.1.contains("organic"), "{}", error.1);
}

#[test]
fn a_mesh_over_the_size_cap_is_refused_before_any_work() {
    let error = evaluate("g = poly.grid@1(width=2, height=2, columns=400, rows=400)\nq = poly.remesh_quads@1(g, faces=500)\noutput o = q\n")
        .err()
        .unwrap();
    assert_eq!(error.0, "poly.remesh_too_large");
    assert!(error.1.contains("200000"), "{}", error.1);
}
