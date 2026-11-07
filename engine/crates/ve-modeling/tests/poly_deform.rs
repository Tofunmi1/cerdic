//! The deformers (bend, twist, taper, shear, wave, noise, lattice) on a polygon mesh: every vertex goes through the point map the solid
//! deformers use, the faces stay as they are. Each result is read back by a `poly.script` over `input`. Written before the body;
//! do not weaken.
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
        transaction: TransactionId::parse("t_poly_deform").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message.clone()))
}

/// A 2 m by 2 m grid of 8 by 8 quads in the XY plane, centred on the origin (x and y run -1 to 1).
const GRID: &str = "g = poly.grid@1(width=2, height=2, columns=8, rows=8)";

fn read(deform: &str, value: &str) -> f32 {
    let source = format!(
        "{GRID}\nd = {deform}\nm = poly.script@1(in=d, code=\"const o = new Mesh(); o.vertex(0,0,0); o.vertex({value},0,0); o.vertex(0,1,0); o.face(0,1,2); o\")\noutput o = m\n"
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

/// JavaScript over `input`: the width (times 1000) of the points on one edge of the grid, `top` (y = 1) or `bottom` (y = -1).
fn width(edge: &str) -> String {
    let test = if edge == "top" {
        "p[1] > 0.99"
    } else {
        "p[1] < -0.99"
    };
    format!(
        "(() => {{ const r = input.points.filter(p => {test}).map(p => p[0]); return (Math.max(...r) - Math.min(...r)) * 1000; }})()"
    )
}

#[test]
fn taper_narrows_the_top_of_a_polygon_grid_and_keeps_the_bottom() {
    let top = read("xform.taper@1(g, top_scale=0.5)", &width("top"));
    let bottom = read("xform.taper@1(g, top_scale=0.5)", &width("bottom"));
    assert!(near(top, 1000.0, 5.0), "the top is half as wide: {top}");
    assert!(
        near(bottom, 2000.0, 5.0),
        "the bottom is unchanged: {bottom}"
    );
}

#[test]
fn shear_slides_the_top_sideways_and_not_the_bottom() {
    let mean_x = |edge: &str| {
        let test = if edge == "top" {
            "p[1] > 0.99"
        } else {
            "p[1] < -0.99"
        };
        format!(
            "(() => {{ const r = input.points.filter(p => {test}); return (r.reduce((s, p) => s + p[0], 0) / r.length + 10) * 1000; }})()"
        )
    };
    let top = read("xform.shear@1(g, x_shift=1)", &mean_x("top"));
    let bottom = read("xform.shear@1(g, x_shift=1)", &mean_x("bottom"));
    assert!(
        near(top, 11_000.0, 10.0),
        "the top row's centre moved the full shift: {top}"
    );
    assert!(
        near(bottom, 10_000.0, 10.0),
        "the bottom row stayed put: {bottom}"
    );
}

#[test]
fn twist_turns_the_top_row_out_of_the_plane() {
    // A quarter turn about Y: the top row, flat along X, ends up along Z.
    let z_extent = "(() => { const r = input.points.filter(p => p[1] > 0.99).map(p => p[2]); return (Math.max(...r) - Math.min(...r)) * 1000; })()";
    let x_extent = "(() => { const r = input.points.filter(p => p[1] > 0.99).map(p => p[0]); return (Math.max(...r) - Math.min(...r)) * 1000; })()";
    let along_z = read("xform.twist@1(g, angle=90)", z_extent);
    let along_x = read("xform.twist@1(g, angle=90)", x_extent);
    assert!(
        along_z > 1500.0,
        "the top row now spans the Z axis: {along_z}"
    );
    assert!(along_x < 500.0, "and hardly the X axis: {along_x}");
}

#[test]
fn bend_curves_the_grid_and_a_too_tight_bend_is_refused_with_the_fix() {
    let reach = read(
        "xform.bend@1(g, angle=60)",
        "(Math.max(...input.points.map(p => p[0])) + 10) * 1000",
    );
    assert!(
        reach > 11_400.0,
        "the far end swings out past the original edge: {reach}"
    );
    let source = "g = poly.grid@1(width=4, height=2, columns=8, rows=8)\nb = xform.bend@1(g, angle=90)\noutput o = b\n";
    let error = evaluate(source).err().unwrap();
    assert_eq!(error.0, "curve.bend_curvature");
}

#[test]
fn wave_ripples_the_grid_sideways() {
    let spread = "(Math.max(...input.points.map(p => p[0])) - Math.min(...input.points.map(p => p[0]))) * 1000";
    let flat = read("xform.move@1(g, by=[0, 0, 0])", spread);
    let waved = read("xform.wave@1(g, amplitude=0.3, wavelength=1)", spread);
    assert!(near(flat, 2000.0, 1.0), "{flat}");
    assert!(
        waved > 2300.0,
        "the crests push the edge out by about 0.3 m: {waved}"
    );
}

#[test]
fn noise_is_repeatable_and_the_seed_changes_it() {
    // The grid is flat, so any depth is the noise: its largest push, in millimetres, plus one.
    let sample = "Math.max(...input.points.map(p => Math.abs(p[2]))) * 1000 + 1";
    let a = read("xform.noise@1(g, amplitude=0.1, scale=0.5, seed=3)", sample);
    let b = read("xform.noise@1(g, amplitude=0.1, scale=0.5, seed=3)", sample);
    let c = read("xform.noise@1(g, amplitude=0.1, scale=0.5, seed=4)", sample);
    assert_eq!(a, b);
    assert!(
        a > 20.0 && a <= 101.5,
        "pushes the surface by up to the amplitude (0.1 m): {a}"
    );
    assert!(
        (a - c).abs() > 0.5,
        "another seed gives another surface: {a} and {c}"
    );
}

#[test]
fn a_lattice_control_point_pulls_the_grid_toward_it() {
    let depth = "(Math.max(...input.points.map(p => Math.abs(p[2]))) + 1) * 1000";
    // The whole middle column of a 3 by 3 by 3 lattice lifted 0.4 m: the grid's centre follows by the middle control point's
    // weight in X and Y (0.5 each) times 0.4, so 0.1 m.
    let all_depths = "1 1 0 0 0 0.4; 1 1 1 0 0 0.4; 1 1 2 0 0 0.4";
    let pulled = read(
        &format!("xform.lattice@1(g, divisions=3, offsets=\"{all_depths}\")"),
        depth,
    );
    let still = read("xform.lattice@1(g, divisions=3)", depth);
    assert!(near(still, 1000.0, 1.0), "no offsets, no movement: {still}");
    assert!(
        near(pulled, 1100.0, 10.0),
        "the middle of the grid is lifted 0.1 m: {pulled}"
    );
}

#[test]
fn the_faces_and_vertices_are_exactly_those_of_the_input() {
    let counts = "input.points.length * 1000 + input.faces.length";
    for deform in [
        "xform.bend@1(g, angle=45)",
        "xform.taper@1(g, top_scale=0.6)",
        "xform.twist@1(g, angle=30)",
    ] {
        assert!(near(read(deform, counts), 81_064.0, 0.5), "{deform}");
    }
}

#[test]
fn a_solid_is_still_refined_and_checked_as_a_solid() {
    let source =
        "b = solid.box@1(size=[1, 2, 1])\nt = xform.taper@1(b, top_scale=0.5)\noutput o = t\n";
    let result = evaluate(source).unwrap();
    let top: Vec<f32> = result.parts[0]
        .positions
        .iter()
        .filter(|p| p[1] > 0.99)
        .map(|p| p[0])
        .collect();
    let width = top.iter().cloned().fold(f32::NEG_INFINITY, f32::max)
        - top.iter().cloned().fold(f32::INFINITY, f32::min);
    assert!(
        near(width, 0.5, 0.01),
        "the box's top is half its width: {width}"
    );
}
