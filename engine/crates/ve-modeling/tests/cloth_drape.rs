//! `cloth.drape`: a flat panel let fall onto a body and settled by the solver. Results are read back by a `poly.script` over `input`.
//! Written before the body; do not weaken.
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
        transaction: TransactionId::parse("t_drape").unwrap(),
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

/// A ball of radius 0.4 centred 0.8 m up, and a 1.2 m square panel of 24 by 24 quads lying flat 0.6 m above its top.
const SCENE: &str = "ball = solid.sphere@1(radius=0.4, segments=48)\nbody = xform.move@1(ball, by=[0, 0.8, 0])\ngrid = poly.grid@1(width=1.2, height=1.2, columns=24, rows=24)\nflat = xform.rotate@1(grid, angles=[-90, 0, 0])\npanel = xform.move@1(flat, by=[0, 1.8, 0])";

#[test]
fn a_panel_settles_over_a_ball_with_its_middle_on_top_and_its_edges_hanging() {
    let setup = format!("{SCENE}\ncloth = cloth.drape@1(panel, with=body, seconds=3)");
    // The highest point rests on the ball's top (0.8 + 0.4) plus the cloth thickness; the lowest hangs well below it.
    let top = read(
        &setup,
        "cloth",
        "Math.max(...input.points.map(p => p[1])) * 1000",
    ) / 1000.0;
    let bottom = read(
        &setup,
        "cloth",
        "(Math.min(...input.points.map(p => p[1])) + 10) * 1000",
    ) / 1000.0
        - 10.0;
    assert!(
        near(top, 1.21, 0.04),
        "the middle sits on top of the ball: {top}"
    );
    assert!(
        bottom < top - 0.3,
        "the corners hang below it: {bottom} against {top}"
    );
}

#[test]
fn no_point_of_the_cloth_is_inside_the_body() {
    let setup = format!("{SCENE}\ncloth = cloth.drape@1(panel, with=body, seconds=3)");
    let deepest = read(
        &setup,
        "cloth",
        "(Math.min(...input.points.map(p => Math.hypot(p[0], p[1] - 0.8, p[2]))) - 0.4 + 10) * 1000",
    ) / 1000.0
        - 10.0;
    assert!(
        deepest > 0.004,
        "every point is outside the 0.4 m sphere by about the thickness: {deepest}"
    );
}

#[test]
fn pinned_points_stay_exactly_where_they_were_and_the_rest_hangs_from_them() {
    let setup = format!(
        "{SCENE}\nedge = poly.select@1(panel, element=\"vertex\", by=\"box\", box_min=[-1, 1.7, -0.61], box_max=[1, 1.9, -0.59])\ncloth = cloth.drape@1(panel, pins=edge, seconds=6)"
    );
    // The panel's back edge (z = -0.6) is held at y = 1.8.
    let held = read(
        &setup,
        "cloth",
        "(input.points.filter(p => Math.abs(p[2] + 0.6) < 1e-4).every(p => Math.abs(p[1] - 1.8) < 1e-4) ? 1 : 0)",
    );
    let hangs = read(
        &setup,
        "cloth",
        "(Math.min(...input.points.map(p => p[1])) + 10) * 1000",
    ) / 1000.0
        - 10.0;
    assert_eq!(held, 1.0, "the pinned edge did not move");
    assert!(
        hangs < 1.8 - 0.8,
        "the rest of the sheet hangs down from it: {hangs}"
    );
}

#[test]
fn a_floor_stops_it_and_it_lies_flat() {
    let setup = "grid = poly.grid@1(width=1, height=1, columns=16, rows=16)\nflat = xform.rotate@1(grid, angles=[-90, 0, 0])\npanel = xform.move@1(flat, by=[0, 0.5, 0])\ncloth = cloth.drape@1(panel, floor=0, seconds=2)";
    let lowest = read(
        setup,
        "cloth",
        "(Math.min(...input.points.map(p => p[1])) + 10) * 1000",
    ) / 1000.0
        - 10.0;
    let highest = read(
        setup,
        "cloth",
        "Math.max(...input.points.map(p => p[1])) * 1000",
    ) / 1000.0;
    assert!(lowest >= 0.005, "nothing sinks into the floor: {lowest}");
    assert!(highest < 0.03, "it lies flat on it: {highest}");
}

#[test]
fn a_panel_that_starts_inside_the_body_is_refused_with_the_fix() {
    let setup = "ball = solid.sphere@1(radius=0.4, segments=32)\ngrid = poly.grid@1(width=1, height=1, columns=12, rows=12)\nflat = xform.rotate@1(grid, angles=[-90, 0, 0])\ncloth = cloth.drape@1(flat, with=ball)\noutput o = cloth\n";
    let error = evaluate(setup).err().unwrap();
    assert_eq!(error.0, "cloth.starts_inside");
    assert!(error.1.contains("inside the body"), "{}", error.1);
}

#[test]
fn an_open_mesh_cannot_be_the_body() {
    let setup = format!(
        "{SCENE}\nopen = poly.grid@1(width=1, height=1, columns=4, rows=4)\ncloth = cloth.drape@1(panel, with=open)\noutput o = cloth\n"
    );
    let error = evaluate(&setup).err().unwrap();
    assert_eq!(error.0, "volume.mesh_open");
}

#[test]
fn the_panels_faces_and_uvs_are_kept() {
    let setup = format!(
        "{SCENE}\ncloth = cloth.drape@1(panel, with=body, seconds=1)\nview = poly.uv_layout@1(cloth)"
    );
    let faces = read(&setup, "cloth", "input.faces.length");
    assert_eq!(faces, 576.0, "24 by 24 quads");
    let source = format!("{setup}\noutput o = view\n");
    let uv = evaluate(&source).unwrap().parts[0].positions.clone();
    let (low, high) = uv
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(l, h), p| {
            (l.min(p[0]), h.max(p[0]))
        });
    assert!(
        near(low, 0.0, 1e-3) && near(high, 1.0, 1e-3),
        "the UV layout is still the unit square: {low}..{high}"
    );
}

#[test]
fn the_same_drape_twice_gives_the_same_mesh() {
    let setup = format!(
        "{SCENE}\ncloth = cloth.drape@1(panel, with=body, seconds=2, drag=0.5, wind=[1, 0, 0])"
    );
    let sum = "input.points.reduce((s, p) => s + p[0] * 3 + p[1] * 5 + p[2] * 7, 0) + 1000";
    assert_eq!(read(&setup, "cloth", sum), read(&setup, "cloth", sum));
}

#[test]
fn a_volume_can_be_the_body_as_well_as_a_mesh() {
    let setup = format!(
        "{SCENE}\nmesh = poly.from_solid@1(body)\nvol = volume.from_mesh@1(mesh, voxel_size=0.03, band=4)\ncloth = cloth.drape@1(panel, with=vol, seconds=3)"
    );
    let top = read(
        &setup,
        "cloth",
        "Math.max(...input.points.map(p => p[1])) * 1000",
    ) / 1000.0;
    assert!(near(top, 1.21, 0.04), "{top}");
}

#[test]
fn a_sheet_dropped_on_a_human_body_stays_on_it_when_it_grips_and_slides_off_when_it_does_not() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/human/human.pack");
    ve_modeling::set_human_pack(&std::fs::read(path).unwrap()).unwrap();
    // A limp sheet balanced on a round head is not a stable thing: with friction it hangs on the body, without enough it slides off to the floor.
    let drape = |friction: f32| {
        format!(
            "body = body.human@1()\ngrid = poly.grid@1(width=1.8, height=1.8, columns=40, rows=40)\nflat = xform.rotate@1(grid, angles=[-90, 0, 0])\nsheet = xform.move@1(flat, by=[0, 2.0, 0])\ncloth = cloth.drape@1(sheet, with=body, seconds=4, voxel_size=0.03, floor=0, friction={friction})"
        )
    };
    let on_body = "input.points.filter(p => p[1] > 0.3 && Math.hypot(p[0], p[2]) < 0.35).length";
    let gripping = read(&drape(1.0), "cloth", on_body);
    let slipping = read(&drape(0.5), "cloth", on_body);
    assert!(
        gripping > 100.0,
        "with friction the sheet is still hanging on the body: {gripping} points near it above the knee"
    );
    assert!(
        slipping < 20.0,
        "without enough it has slid off: {slipping} points"
    );
    let highest = read(
        &drape(1.0),
        "cloth",
        "(Math.max(...input.points.map(p => p[1])) + 100) * 1000",
    ) / 1000.0
        - 100.0;
    let lowest = read(
        &drape(1.0),
        "cloth",
        "(Math.min(...input.points.map(p => p[1])) + 100) * 1000",
    ) / 1000.0
        - 100.0;
    assert!(
        highest > 1.2 && highest < 1.95,
        "its top is on the head or shoulders: {highest}"
    );
    assert!(lowest >= 0.005, "and nothing is below the floor: {lowest}");
}

#[test]
fn a_panel_with_too_many_points_is_refused_with_the_limit() {
    let setup = "grid = poly.grid@1(width=2, height=2, columns=200, rows=200)\ncloth = cloth.drape@1(grid, floor=0)\noutput o = cloth\n";
    let error = evaluate(setup).err().unwrap();
    assert_eq!(error.0, "budget.cloth");
    assert!(error.1.contains("20000"), "{}", error.1);
}

#[test]
fn wind_blows_a_pinned_sheet_downwind() {
    let sheet = |wind: &str| {
        format!(
            "{SCENE}\nedge = poly.select@1(panel, element=\"vertex\", by=\"box\", box_min=[-1, 1.7, -0.61], box_max=[1, 1.9, -0.59])\ncloth = cloth.drape@1(panel, pins=edge, seconds=4, drag=1, wind={wind})"
        )
    };
    let mean_x = "(input.points.reduce((s, p) => s + p[0], 0) / input.points.length + 10) * 1000";
    let calm = read(&sheet("[0, 0, 0]"), "cloth", mean_x) / 1000.0 - 10.0;
    let blown = read(&sheet("[6, 0, 0]"), "cloth", mean_x) / 1000.0 - 10.0;
    assert!(
        blown > calm + 0.05,
        "wind along +x pushes it that way: {calm} then {blown}"
    );
}
