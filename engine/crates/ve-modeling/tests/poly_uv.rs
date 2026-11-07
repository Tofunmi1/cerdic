//! UV editing and viewing: `poly.uv_transform` (shift, scale, turn; all UVs or one island) and `poly.uv_layout` (the layout as a flat mesh).
//! The layout is read through `poly.uv_layout`, whose vertices are the UVs. Written before the body; do not weaken.
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
        transaction: TransactionId::parse("t_poly_uv").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message.clone()))
}

/// The layout's points (x = u, y = v) for the program, which must end by giving the mesh `uvs`.
fn layout(setup: &str) -> Vec<[f32; 3]> {
    evaluate(&format!(
        "{setup}\nview = poly.uv_layout@1(uvs)\noutput o = view\n"
    ))
    .unwrap()
    .parts[0]
        .positions
        .clone()
}

fn bounds(points: &[[f32; 3]]) -> ([f32; 2], [f32; 2]) {
    let mut low = [f32::INFINITY; 2];
    let mut high = [f32::NEG_INFINITY; 2];
    for p in points {
        for axis in 0..2 {
            low[axis] = low[axis].min(p[axis]);
            high[axis] = high[axis].max(p[axis]);
        }
    }
    (low, high)
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

const GRID: &str = "g = poly.grid@1(width=2, height=1, columns=2, rows=1)";

#[test]
fn the_layout_of_a_grid_fills_the_unit_square() {
    let (low, high) = bounds(&layout(&format!("{GRID}\nuvs = poly.uv_transform@1(g)")));
    assert!(
        near(low[0], 0.0) && near(low[1], 0.0) && near(high[0], 1.0) && near(high[1], 1.0),
        "{low:?} {high:?}"
    );
}

#[test]
fn a_layout_size_stretches_the_whole_uv_square() {
    let source = format!("{GRID}\nview = poly.uv_layout@1(g, size=2)\noutput o = view\n");
    let points = evaluate(&source).unwrap().parts[0].positions.clone();
    let (low, high) = bounds(&points);
    assert!(
        near(high[0] - low[0], 2.0) && near(high[1] - low[1], 2.0),
        "{low:?} {high:?}"
    );
}

#[test]
fn offset_shifts_every_uv() {
    let (low, high) = bounds(&layout(&format!(
        "{GRID}\nuvs = poly.uv_transform@1(g, offset_u=0.5, offset_v=-0.25)"
    )));
    assert!(near(low[0], 0.5) && near(high[0], 1.5), "{low:?} {high:?}");
    assert!(
        near(low[1], -0.25) && near(high[1], 0.75),
        "{low:?} {high:?}"
    );
}

#[test]
fn scale_shrinks_the_layout_about_its_own_centre() {
    let (low, high) = bounds(&layout(&format!(
        "{GRID}\nuvs = poly.uv_transform@1(g, scale=0.5)"
    )));
    assert!(
        near(low[0], 0.25) && near(high[0], 0.75) && near(low[1], 0.25) && near(high[1], 0.75),
        "{low:?} {high:?}"
    );
}

#[test]
fn rotate_about_the_origin_turns_the_square_into_the_next_quadrant() {
    let (low, high) = bounds(&layout(&format!(
        "{GRID}\nuvs = poly.uv_transform@1(g, rotate=90, pivot=\"origin\")"
    )));
    assert!(
        near(low[0], -1.0) && near(high[0], 0.0) && near(low[1], 0.0) && near(high[1], 1.0),
        "{low:?} {high:?}"
    );
}

#[test]
fn a_selection_moves_only_that_islands_faces() {
    // The grid's two faces share the unit square, left half u 0 to 0.5 and right half 0.5 to 1. Lift the left face.
    let setup = format!(
        "{GRID}\nleft = poly.select@1(g, element=\"face\", by=\"box\", box_min=[-2, -1, -1], box_max=[0, 1, 1])\nuvs = poly.uv_transform@1(g, left, offset_v=0.2)"
    );
    let points = layout(&setup);
    // The outer edges belong to one face each: u = 0 is only the left face's, u = 1 only the right face's.
    let left: Vec<[f32; 3]> = points.iter().copied().filter(|p| p[0] < 0.01).collect();
    let right: Vec<[f32; 3]> = points.iter().copied().filter(|p| p[0] > 0.99).collect();
    assert!(!left.is_empty() && !right.is_empty());
    let (ll, lh) = bounds(&left);
    let (rl, rh) = bounds(&right);
    assert!(
        near(ll[1], 0.2) && near(lh[1], 1.2),
        "the left face moved up by 0.2: {ll:?} {lh:?}"
    );
    assert!(
        near(rl[1], 0.0) && near(rh[1], 1.0),
        "the right face did not move: {rl:?} {rh:?}"
    );
}

#[test]
fn the_original_mesh_is_untouched_by_the_layout_view() {
    let source = format!(
        "{GRID}\nview = poly.uv_layout@1(g, size=5)\noutput view = view\noutput original = g\n"
    );
    let result = evaluate(&source).unwrap();
    let original = result.parts.iter().find(|p| p.name == "original").unwrap();
    let (low, high) = bounds(&original.positions);
    assert!(near(low[0], -1.0) && near(high[0], 1.0), "{low:?} {high:?}");
}

#[test]
fn a_mesh_without_uvs_is_refused_with_the_fix() {
    let source = "m = poly.script@1(code=\"const m = new Mesh(); m.vertex(0,0,0); m.vertex(1,0,0); m.vertex(0,1,0); m.face(0,1,2); m\")\nt = poly.uv_transform@1(m, offset_u=1)\noutput o = t\n";
    let error = evaluate(source).err().unwrap();
    assert_eq!(error.0, "poly.uv_missing");
    assert!(error.1.contains("poly.unwrap_uv"), "{}", error.1);
}

#[test]
fn a_selection_of_edges_is_refused_with_what_is_needed() {
    let source = format!(
        "{GRID}\nedges = poly.select@1(g, element=\"edge\", by=\"all\")\nt = poly.uv_transform@1(g, edges, offset_u=1)\noutput o = t\n"
    );
    let error = evaluate(&source).err().unwrap();
    assert_eq!(error.0, "poly.selection_kind");
    assert!(error.1.contains("faces"), "{}", error.1);
}

#[test]
fn an_unwrapped_mesh_can_be_viewed_as_islands_with_gaps_between_them() {
    let setup =
        "b = solid.box@1(size=[1, 1, 1])\nm = poly.from_solid@1(b)\nuvs = poly.unwrap_uv@1(m)";
    let points = layout(setup);
    let (low, high) = bounds(&points);
    assert!(
        low[0] >= -1e-3 && low[1] >= -1e-3 && high[0] <= 1.001 && high[1] <= 1.001,
        "a packed layout stays in the unit square: {low:?} {high:?}"
    );
    assert!(
        points.len() >= 24,
        "a box's islands have at least 24 corners: {}",
        points.len()
    );
}
