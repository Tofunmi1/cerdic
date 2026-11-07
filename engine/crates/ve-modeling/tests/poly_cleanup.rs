//! The whole-mesh cleanup tools (recalc normals, smooth, fill holes, beautify, planar faces, join triangles, dissolve degenerate).
//! Each is measured by a `poly.script` that calls the node as a function and returns the number it measured as the x extent of a
//! tiny mesh. Written before the body; do not weaken.
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
        transaction: TransactionId::parse("t_poly_cleanup").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message.clone()))
}

/// Runs `body` (JavaScript that ends by calling `out(number)`) and reads the number back.
fn measure(body: &str) -> f32 {
    let helper = "const out = (v) => { const m = new Mesh(); m.vertex(0, 0, 0); m.vertex(v, 0, 0); m.vertex(0, 1, 0); m.face(0, 1, 2); return m; };";
    let source = format!("s = poly.script@1(code=\"{helper} {body}\")\noutput o = s\n");
    let result = evaluate(&source).unwrap();
    result.parts[0]
        .positions
        .iter()
        .map(|p| p[0])
        .fold(f32::NEG_INFINITY, f32::max)
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

/// A unit cube with faces 2 and 4 wound the wrong way, as JavaScript source defining `cube`.
const MIXED_CUBE: &str = "const cube = new Mesh(); [[0,0,0],[1,0,0],[1,1,0],[0,1,0],[0,0,1],[1,0,1],[1,1,1],[0,1,1]].forEach(p => cube.vertex(p[0], p[1], p[2])); [[0,3,2,1],[4,5,6,7],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7]].forEach((f, i) => cube.face(...((i === 2 || i === 4) ? f.slice().reverse() : f)));";

/// JavaScript defining `facing(mesh)`: how many faces point away from the mesh's centre.
const FACING: &str = "const facing = (m) => { const c = [0, 0, 0]; m.points.forEach(p => { c[0] += p[0] / m.points.length; c[1] += p[1] / m.points.length; c[2] += p[2] / m.points.length; }); let outward = 0; m.faces.forEach(f => { const a = m.points[f[0]], b = m.points[f[1]], d = m.points[f[2]]; const u = [b[0]-a[0], b[1]-a[1], b[2]-a[2]], v = [d[0]-a[0], d[1]-a[1], d[2]-a[2]]; const n = [u[1]*v[2]-u[2]*v[1], u[2]*v[0]-u[0]*v[2], u[0]*v[1]-u[1]*v[0]]; const mid = [(a[0]+b[0]+d[0])/3 - c[0], (a[1]+b[1]+d[1])/3 - c[1], (a[2]+b[2]+d[2])/3 - c[2]]; if (n[0]*mid[0] + n[1]*mid[1] + n[2]*mid[2] > 0) outward++; }); return outward; };";

#[test]
fn recalc_normals_points_every_face_outward() {
    let before = measure(&format!("{MIXED_CUBE} {FACING} out(facing(cube));"));
    let after = measure(&format!(
        "{MIXED_CUBE} {FACING} out(facing(op('poly.recalc_normals@1', {{direction: 'outward'}}, {{in: cube}})));"
    ));
    assert!(
        near(before, 4.0, 0.01),
        "two of six faces start inside out: {before}"
    );
    assert!(
        near(after, 6.0, 0.01),
        "all six face outward after: {after}"
    );
}

#[test]
fn recalc_normals_can_point_every_face_inward() {
    let inward = measure(&format!(
        "{MIXED_CUBE} {FACING} out(facing(op('poly.recalc_normals@1', {{direction: 'inward'}}, {{in: cube}})));"
    ));
    assert!(near(inward, 0.0, 0.01), "no face points outward: {inward}");
}

#[test]
fn smooth_lowers_a_spike_and_more_passes_lower_it_more() {
    let spike = "const g = op('poly.grid@1', {width: 4, height: 4, columns: 8, rows: 8}); const spiked = { points: g.points.map((p, i) => i === 40 ? [p[0], p[1], 1] : p), faces: g.faces }; const peak = (m) => Math.max(...m.points.map(p => p[2]));";
    let none = measure(&format!("{spike} out(peak(spiked));"));
    let one = measure(&format!(
        "{spike} out(peak(op('poly.smooth@1', {{factor: 0.5, iterations: 1}}, {{in: spiked}})));"
    ));
    let five = measure(&format!(
        "{spike} out(peak(op('poly.smooth@1', {{factor: 0.5, iterations: 5}}, {{in: spiked}})));"
    ));
    assert!(near(none, 1.0, 1e-4), "{none}");
    assert!(one < none - 0.1, "one pass lowers the spike: {one}");
    assert!(
        five < one - 0.05,
        "five passes lower it further: {five} vs {one}"
    );
}

#[test]
fn fill_holes_closes_a_hole_only_up_to_its_size_limit() {
    let open_box = "const m = new Mesh(); [[0,0,0],[1,0,0],[1,1,0],[0,1,0],[0,0,1],[1,0,1],[1,1,1],[0,1,1]].forEach(p => m.vertex(p[0], p[1], p[2])); [[0,3,2,1],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7]].forEach(f => m.face(...f));";
    let filled = measure(&format!(
        "{open_box} out(op('poly.fill_holes@1', {{max_sides: 4}}, {{in: m}}).faces.length);"
    ));
    let left_open = measure(&format!(
        "{open_box} out(op('poly.fill_holes@1', {{max_sides: 3}}, {{in: m}}).faces.length);"
    ));
    assert!(
        near(filled, 6.0, 0.01),
        "the four-sided hole is filled: {filled}"
    );
    assert!(
        near(left_open, 5.0, 0.01),
        "a limit of three sides leaves it open: {left_open}"
    );
}

#[test]
fn beautify_by_area_flips_the_shared_diagonal_of_a_triangle_pair() {
    // A(0,0) B(3,0) C(4,2) D(0,1) starts split along A-C; Blender's area measure prefers B-D here (the angle measure keeps A-C on this
    // quad, so it is not asserted: which diagonal is "better" is Blender's measure, not ours).
    let quad = "const m = new Mesh(); m.vertex(0,0,0); m.vertex(3,0,0); m.vertex(4,2,0); m.vertex(0,1,0); m.face(0,1,2); m.face(0,2,3); const hasEdge = (r, p, q) => r.faces.some(f => f.some(i => r.points[i][0] === p[0] && r.points[i][1] === p[1]) && f.some(i => r.points[i][0] === q[0] && r.points[i][1] === q[1]));";
    // Result encodes: 10 if the better diagonal B-D is present, plus 1 if the worse diagonal A-C is present.
    let describe = "const both = (r) => (hasEdge(r, [3,0], [0,1]) ? 10 : 0) + (hasEdge(r, [0,0], [4,2]) ? 1 : 0);";
    let before = measure(&format!(
        "{quad} {describe} out(both({{points: m.points, faces: m.faces}}));"
    ));
    let after = measure(&format!(
        "{quad} {describe} out(both(op('poly.beautify@1', {{method: 'area'}}, {{in: m}})));"
    ));
    assert!(
        near(before, 1.0, 0.01),
        "starts split along A-C only: {before}"
    );
    assert!(
        near(after, 10.0, 0.01),
        "ends split along B-D only: {after}"
    );
}

#[test]
fn planar_faces_flattens_a_bent_quad() {
    let bent = "const m = new Mesh(); m.vertex(0,0,0); m.vertex(1,0,0); m.vertex(1,1,0.4); m.vertex(0,1,0); m.face(0,1,2,3); const off = (r) => { const [a, b, c, d] = r.points; const u = [b[0]-a[0], b[1]-a[1], b[2]-a[2]], v = [d[0]-a[0], d[1]-a[1], d[2]-a[2]]; const n = [u[1]*v[2]-u[2]*v[1], u[2]*v[0]-u[0]*v[2], u[0]*v[1]-u[1]*v[0]]; const len = Math.hypot(...n); return Math.abs(((c[0]-a[0])*n[0] + (c[1]-a[1])*n[1] + (c[2]-a[2])*n[2]) / len); };";
    let before = measure(&format!("{bent} out(off({{points: m.points}}) * 1000);"));
    let after = measure(&format!(
        "{bent} out(off(op('poly.planar_faces@1', {{iterations: 10, factor: 1}}, {{in: m}})) * 1000);"
    ));
    assert!(
        before > 300.0,
        "the fourth corner starts 0.4 m off the plane: {before}"
    );
    assert!(
        after < 1.0,
        "afterwards all four corners are on one plane: {after}"
    );
}

#[test]
fn join_triangles_turns_a_triangulated_grid_back_into_quads() {
    let count = measure(
        "const g = op('poly.grid@1', {width: 4, height: 4, columns: 4, rows: 4}); const t = op('poly.triangulate@1', {}, {in: g}); const j = op('poly.join_triangles@1', {face_angle: 0.5, shape_angle: 0.5}, {in: t}); out(t.faces.length * 100 + j.faces.length);",
    );
    assert!(
        near(count, 3216.0, 0.5),
        "32 triangles join into 16 quads: {count}"
    );
}

#[test]
fn dissolve_degenerate_removes_a_sliver_triangle_with_a_micrometre_edge() {
    // Two triangles share a corner; the first has a micrometre edge (1,0)-(1,0.000001), so it is a sliver, not a real face.
    let slivers = "const m = new Mesh(); m.vertex(0,0,0); m.vertex(1,0,0); m.vertex(1,0.000001,0); m.vertex(0,1,0); m.face(0,1,2); m.face(0,2,3);";
    let faces = measure(&format!(
        "{slivers} const r = op('poly.dissolve_degenerate@1', {{distance: 0.0001}}, {{in: m}}); out(r.faces.length);"
    ));
    assert!(
        near(faces, 1.0, 0.01),
        "two faces become the one real triangle: {faces}"
    );
}

#[test]
fn a_bad_parameter_is_refused_with_the_reason() {
    let source = "s = poly.script@1(code=\"op('poly.smooth@1', {factor: 2}, {in: op('poly.grid@1', {width: 1, height: 1})}); new Mesh()\")\noutput o = s\n";
    let error = evaluate(source).err().unwrap();
    assert_eq!(error.0, "script.error");
    assert!(error.1.contains("poly.smooth@1"), "{}", error.1);
}
