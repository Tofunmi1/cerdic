//! Volume grids (`volume.from_mesh`, `combine`, `smooth`, `offset`, `to_mesh`): meshes in, measured meshes out. Every result is read back by a
//! `poly.script` over `input`, which reports a number as the x extent of a tiny mesh. Written before the body; do not weaken.
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
        transaction: TransactionId::parse("t_volume").unwrap(),
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

/// JavaScript over `input`: the enclosed volume (in litres, so 1 m3 reads 1000) plus 100000, from the faces fanned into triangles.
const VOLUME: &str = "(() => { let v = 0; input.faces.forEach(f => { for (let k = 1; k + 1 < f.length; k++) { const a = input.points[f[0]], b = input.points[f[k]], c = input.points[f[k + 1]]; v += a[0]*(b[1]*c[2]-b[2]*c[1]) - a[1]*(b[0]*c[2]-b[2]*c[0]) + a[2]*(b[0]*c[1]-b[1]*c[0]); } }); return v / 6 * 1000 + 100000; })()";
/// JavaScript over `input`: 1 when every edge joins exactly two faces.
const CLOSED: &str = "(() => { const e = new Map(); input.faces.forEach(f => f.forEach((a, i) => { const b = f[(i + 1) % f.length]; const k = Math.min(a, b) + ':' + Math.max(a, b); e.set(k, (e.get(k) || 0) + 1); })); return [...e.values()].every(n => n === 2) ? 1 : 0; })()";
/// JavaScript over `input`: the share of faces that are quads, in thousandths.
const QUAD_SHARE: &str =
    "(input.faces.filter(f => f.length === 4).length / input.faces.length * 1000)";

fn litres(setup: &str, node: &str) -> f32 {
    read(setup, node, VOLUME) - 100_000.0
}

const SPHERE: &str = "s = solid.sphere@1(radius=1, segments=48)\nsrc = poly.from_solid@1(s)\nv = volume.from_mesh@1(src, voxel_size=0.1)\nback = volume.to_mesh@1(v)";
const SPHERE_LITRES: f32 = 4188.79;

#[test]
fn a_sphere_goes_into_a_volume_and_comes_back_as_a_closed_nearly_all_quad_mesh_of_the_right_size() {
    assert_eq!(
        read(SPHERE, "back", CLOSED),
        1.0,
        "every edge joins two faces"
    );
    assert!(
        read(SPHERE, "back", QUAD_SHARE) > 950.0,
        "nearly every face is a quad"
    );
    let size = litres(SPHERE, "back");
    assert!(
        size > 0.0,
        "it faces outward, so the volume is positive: {size}"
    );
    assert!(
        near(size, SPHERE_LITRES, SPHERE_LITRES * 0.04),
        "a unit sphere is 4189 litres, got {size}"
    );
}

#[test]
fn the_meshed_surface_lies_on_the_original_within_a_voxel() {
    let radial = "(() => { const r = input.points.map(p => Math.hypot(...p)); return Math.max(Math.abs(Math.max(...r) - 1), Math.abs(Math.min(...r) - 1)) * 1000 + 1; })()";
    let error = read(SPHERE, "back", radial) - 1.0;
    assert!(
        error < 30.0,
        "within 3 cm of the unit sphere with 10 cm voxels: {error} thousandths"
    );
}

#[test]
fn a_box_keeps_its_size_though_its_corners_soften() {
    let setup = "b = solid.box@1(size=[1, 1, 1])\nv = volume.from_mesh@1(b, voxel_size=0.05)\nback = volume.to_mesh@1(v)";
    let size = litres(setup, "back");
    assert!(
        near(size, 1000.0, 90.0),
        "a unit cube is 1000 litres, got {size}"
    );
}

fn two_spheres(operation: &str, extra: &str) -> String {
    format!(
        "a = solid.sphere@1(radius=1, segments=48)\nb = solid.sphere@1(radius=1, segments=48)\nb2 = xform.move@1(b, by=[1, 0, 0])\nma = poly.from_solid@1(a)\nmb = poly.from_solid@1(b2)\nva = volume.from_mesh@1(ma, voxel_size=0.1)\nvb = volume.from_mesh@1(mb, voxel_size=0.1)\nc = volume.combine@1(va, vb, operation=\"{operation}\"{extra})\nback = volume.to_mesh@1(c)"
    )
}

#[test]
fn union_covers_both_shapes() {
    let setup = two_spheres("union", "");
    let wide = read(
        &setup,
        "back",
        "(Math.max(...input.points.map(p => p[0])) - Math.min(...input.points.map(p => p[0])) + 100) * 1000",
    );
    assert!(
        near(wide / 1000.0 - 100.0, 3.0, 0.12),
        "from -1 to 2 along x: {}",
        wide / 1000.0 - 100.0
    );
}

#[test]
fn intersect_keeps_only_the_lens_between_them() {
    // Two unit spheres a metre apart overlap in a lens of volume pi (4r + d)(2r - d)^2 / 12 = 1.309 m3.
    let lens = litres(&two_spheres("intersect", ""), "back");
    assert!(near(lens, 1309.0, 130.0), "{lens} litres");
}

#[test]
fn subtract_takes_the_second_out_of_the_first() {
    // Half of the first sphere is cut away: the remaining volume is the sphere minus the lens.
    let left = litres(&two_spheres("subtract", ""), "back");
    assert!(near(left, 4189.0 - 1309.0, 260.0), "{left} litres");
}

#[test]
fn a_subtracted_cavity_is_inside_out_and_counts_against_the_volume() {
    let setup = "b = solid.box@1(size=[2, 2, 2])\nh = solid.sphere@1(radius=0.6, segments=32)\nmb = poly.from_solid@1(b)\nmh = poly.from_solid@1(h)\nvb = volume.from_mesh@1(mb, voxel_size=0.08)\nvh = volume.from_mesh@1(mh, voxel_size=0.08)\nc = volume.combine@1(vb, vh, operation=\"subtract\")\nback = volume.to_mesh@1(c)";
    let size = litres(setup, "back");
    assert!(
        near(size, 8000.0 - 904.8, 450.0),
        "a 2 m cube less a 0.6 m sphere is 7095 litres, got {size}"
    );
}

#[test]
fn a_blend_adds_material_where_shapes_meet() {
    let hard = litres(&two_spheres("union", ""), "back");
    let soft = litres(&two_spheres("union", ", blend=0.4"), "back");
    assert!(
        soft > hard + 20.0,
        "melting the join together adds material: {hard} then {soft}"
    );
}

#[test]
fn offset_grows_and_shrinks_by_the_distance() {
    let radius = "(() => { const r = input.points.map(p => Math.hypot(...p)); return (r.reduce((s, x) => s + x, 0) / r.length) * 1000; })()";
    let with = |d: f32| {
        format!(
            "s = solid.sphere@1(radius=1, segments=48)\nsrc = poly.from_solid@1(s)\nv = volume.from_mesh@1(src, voxel_size=0.1, band=8)\no = volume.offset@1(v, distance={d})\nback = volume.to_mesh@1(o)"
        )
    };
    let grown = read(&with(0.2), "back", radius) / 1000.0;
    let shrunk = read(&with(-0.3), "back", radius) / 1000.0;
    assert!(near(grown, 1.2, 0.03), "{grown}");
    assert!(near(shrunk, 0.7, 0.03), "{shrunk}");
}

#[test]
fn smoothing_rounds_the_corners_of_a_box() {
    let corner = "(Math.max(...input.points.map(p => Math.hypot(...p))) * 1000)";
    let setup = |passes: u32| {
        format!(
            "b = solid.box@1(size=[1, 1, 1])\nv = volume.from_mesh@1(b, voxel_size=0.04)\ns = volume.smooth@1(v, iterations={passes}, factor=1)\nback = volume.to_mesh@1(s)"
        )
    };
    let sharp = read(&setup(1), "back", corner) / 1000.0;
    let soft = read(&setup(12), "back", corner) / 1000.0;
    assert!(
        soft < sharp - 0.03,
        "the farthest corner pulls in: {sharp} then {soft}"
    );
}

#[test]
fn the_same_input_gives_the_same_mesh() {
    let sum = "input.points.reduce((s, p) => s + p[0] * 3 + p[1] * 5 + p[2] * 7, 0) + 1000";
    assert_eq!(read(SPHERE, "back", sum), read(SPHERE, "back", sum));
}

#[test]
fn an_open_mesh_is_refused_with_how_many_edges_are_loose_and_the_fix() {
    let error = evaluate("g = poly.grid@1(width=1, height=1, columns=3, rows=3)\nv = volume.from_mesh@1(g)\nback = volume.to_mesh@1(v)\noutput o = back\n").err().unwrap();
    assert_eq!(error.0, "volume.mesh_open");
    assert!(
        error.1.contains("not closed") && error.1.contains("12 edges"),
        "{}",
        error.1
    );
}

#[test]
fn volumes_of_different_voxel_sizes_cannot_be_combined() {
    let setup = "s = solid.sphere@1(radius=1, segments=24)\nm = poly.from_solid@1(s)\na = volume.from_mesh@1(m, voxel_size=0.1)\nb = volume.from_mesh@1(m, voxel_size=0.2)\nc = volume.combine@1(a, b)\nback = volume.to_mesh@1(c)\noutput o = back\n";
    let error = evaluate(setup).err().unwrap();
    assert_eq!(error.0, "volume.cell_mismatch");
    assert!(
        error.1.contains("voxel sizes") && error.1.contains("0.2000"),
        "{}",
        error.1
    );
}

#[test]
fn an_offset_beyond_the_band_is_refused_with_the_limit() {
    let error = evaluate("s = solid.sphere@1(radius=1, segments=24)\nm = poly.from_solid@1(s)\nv = volume.from_mesh@1(m, voxel_size=0.1, band=3)\no = volume.offset@1(v, distance=0.5)\nback = volume.to_mesh@1(o)\noutput o = back\n").err().unwrap();
    assert_eq!(error.0, "volume.offset_range");
    assert!(error.1.contains("0.3"), "the grid holds 0.3 m: {}", error.1);
}

#[test]
fn a_grid_over_the_voxel_budget_is_refused_naming_the_size() {
    let error = evaluate("s = solid.box@1(size=[20, 20, 20])\nv = volume.from_mesh@1(s, voxel_size=0.02)\nback = volume.to_mesh@1(v)\noutput o = back\n").err().unwrap();
    assert_eq!(error.0, "budget.volume");
    assert!(error.1.contains("voxel"), "{}", error.1);
}

#[test]
fn a_volume_cannot_be_a_scene_output_and_the_message_says_how_to_mesh_it() {
    let error = evaluate("s = solid.sphere@1(radius=1, segments=24)\nm = poly.from_solid@1(s)\nv = volume.from_mesh@1(m)\noutput o = v\n").err().unwrap();
    assert_eq!(error.0, "graph.port_type");
    assert!(
        error.1.contains("volume.to_mesh") || error.1.contains("volume"),
        "{}",
        error.1
    );
}
