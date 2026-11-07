//! The selection-based mesh tools: split edges, rotate edges, connect vertices. Each runs behind a `poly.select` and is read back by a
//! `poly.script` that encodes what it found in `input` as the x extent of a tiny mesh. Written before the body; do not weaken.
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
        transaction: TransactionId::parse("t_poly_edges").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message.clone()))
}

/// Reads `node` back: `value` is JavaScript over `input` giving a number, returned as the x extent of the output.
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

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

const BOX: &str =
    "b = solid.box@1(size=[1, 1, 1])\nf = poly.from_solid@1(b)\nmc = poly.merge_coplanar@1(f)";
const COUNTS: &str = "input.points.length * 100 + input.faces.length";

#[test]
fn split_edges_gives_each_side_of_a_hard_edge_its_own_vertices() {
    let before = read(BOX, "mc", COUNTS);
    let after = read(
        &format!(
            "{BOX}\nsel = poly.select@1(mc, element=\"edge\", by=\"sharp\")\nsp = poly.split_edges@1(mc, sel)"
        ),
        "sp",
        COUNTS,
    );
    assert!(
        near(before, 806.0),
        "a box has 8 vertices and 6 faces: {before}"
    );
    assert!(
        near(after, 2406.0),
        "split along every edge: 24 vertices (3 per corner), still 6 faces: {after}"
    );
}

#[test]
fn split_edges_refuses_a_selection_of_faces_and_says_what_it_needs() {
    let error = evaluate(&format!(
        "{BOX}\nsel = poly.select@1(mc, element=\"face\", by=\"all\")\nsp = poly.split_edges@1(mc, sel)\noutput o = sp\n"
    ))
    .err()
    .unwrap();
    assert_eq!(error.0, "poly.selection_kind");
    assert!(error.1.contains("edge"), "{}", error.1);
}

/// JavaScript over `input` (two triangles of a unit quad): 1 when the shared edge runs corner to corner through (-,-) and (+,+), 2 for the other diagonal.
const WHICH_DIAGONAL: &str = "(() => { const key = (f) => f.map(i => i); const shared = input.faces[0].filter(i => input.faces[1].includes(i)); if (shared.length !== 2) return 9; const p = input.points[shared[0]]; return p[0] * p[1] > 0 ? 1 : 2; })()";

#[test]
fn rotate_edges_turns_the_shared_edge_of_a_triangle_pair_to_the_other_diagonal() {
    let setup = "g = poly.grid@1(width=1, height=1, columns=1, rows=1)\nt = poly.triangulate@1(g)\nsel = poly.select@1(t, element=\"edge\", by=\"box\", box_min=[-0.1, -0.1, -0.1], box_max=[0.1, 0.1, 0.1])\nr = poly.rotate_edges@1(t, sel)";
    let before = read(setup, "t", WHICH_DIAGONAL);
    let after = read(setup, "r", WHICH_DIAGONAL);
    assert!(
        before == 1.0 || before == 2.0,
        "the quad starts as two triangles sharing one diagonal: {before}"
    );
    assert!(
        after == 1.0 || after == 2.0,
        "and ends the same way: {after}"
    );
    assert!(
        (before - after).abs() > 0.5,
        "the diagonal changed: {before} then {after}"
    );
}

#[test]
fn connect_vertices_cuts_a_hexagon_in_two_along_a_diameter() {
    let setup = "hex = poly.script@1(code=\"const m = new Mesh(); for (let i = 0; i < 6; i++) m.vertex(Math.cos(i * Math.PI / 3), Math.sin(i * Math.PI / 3), 0); m.face(0,1,2,3,4,5); m\")\nsel = poly.select@1(hex, element=\"vertex\", by=\"box\", box_min=[-2, -0.1, -1], box_max=[2, 0.1, 1])\nc = poly.connect_vertices@1(hex, sel)";
    let whole = read(
        setup,
        "hex",
        "input.points.length * 100 + input.faces.length * 10 + input.faces[0].length",
    );
    let cut = read(
        setup,
        "c",
        "input.points.length * 100 + input.faces.length * 10 + input.faces[0].length",
    );
    assert!(near(whole, 616.0), "one six-sided face: {whole}");
    assert!(
        near(cut, 624.0),
        "the same six vertices, two four-sided faces: {cut}"
    );
}

#[test]
fn connecting_a_single_vertex_is_refused() {
    let error = evaluate(
        "hex = poly.script@1(code=\"const m = new Mesh(); for (let i = 0; i < 6; i++) m.vertex(Math.cos(i * Math.PI / 3), Math.sin(i * Math.PI / 3), 0); m.face(0,1,2,3,4,5); m\")\nsel = poly.select@1(hex, element=\"vertex\", by=\"box\", box_min=[0.9, -0.1, -1], box_max=[2, 0.1, 1])\nc = poly.connect_vertices@1(hex, sel)\noutput o = c\n",
    )
    .err()
    .unwrap();
    assert_eq!(error.0, "poly.connect_needs_two_vertices");
    assert!(error.1.contains("at least two"), "{}", error.1);
}

#[test]
fn connect_vertices_refuses_a_selection_of_faces() {
    let error = evaluate(
        "g = poly.grid@1(width=1, height=1, columns=2, rows=2)\nsel = poly.select@1(g, element=\"face\", by=\"all\")\nc = poly.connect_vertices@1(g, sel)\noutput o = c\n",
    )
    .err()
    .unwrap();
    assert_eq!(error.0, "poly.selection_kind");
    assert!(error.1.contains("vertex"), "{}", error.1);
}
