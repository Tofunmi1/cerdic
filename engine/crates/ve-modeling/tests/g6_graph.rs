//! G6 through the node language: polygon sources, selections and edits evaluated end to end.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "the helper unwraps fixtures that are known to be valid and reports failures by panicking"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn session(text: &str) -> Session {
    let batch = parse_text(&builtin_registry(), text).unwrap();
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g6_graph").unwrap(),
        label: None,
        commands: batch.commands,
    });
    assert!(
        matches!(outcome, Outcome::Committed { .. }),
        "commits: {outcome:?}"
    );
    session
}

fn evaluate(text: &str) -> Result<Evaluation, (String, String)> {
    Evaluator::default()
        .evaluate(session(text).document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message))
}

fn ok(text: &str) -> Evaluation {
    evaluate(text).unwrap_or_else(|(code, message)| panic!("{code}: {message}"))
}

fn refused(text: &str) -> String {
    evaluate(text).err().map(|(code, _)| code).unwrap()
}

#[test]
fn g6_01_a_solid_survives_a_round_trip_through_a_polygon_mesh() {
    let result = ok("box = solid.box@1(size=[1, 1, 1])\n\
         mesh = poly.from_solid@1(box)\n\
         back = poly.to_solid@1(mesh)\n\
         output back");
    assert!((result.parts[0].volume - 1.0).abs() < 1.0e-6);
}

#[test]
fn g6_03_a_raised_panel_is_made_by_selecting_dissolving_insetting_and_extruding() {
    // Merge the top's two triangles into a quad, inset it, pick the inner cap by its role tag, and
    // push it out: a unit box with a 0.8 x 0.8 panel standing 0.2 above it.
    let result = ok("box = solid.box@1(size=[1, 1, 1])\n\
         mesh = poly.from_solid@1(box)\n\
         top = poly.select@1(mesh, by=\"normal\", direction=[0, 1, 0], angle=0.05)\n\
         quad = poly.dissolve_faces@1(mesh, top)\n\
         face = poly.select@1(quad, by=\"normal\", direction=[0, 1, 0], angle=0.05, expect=\"one\")\n\
         inset = poly.inset_faces@1(quad, face, distance=0.1)\n\
         cap = poly.select@1(inset, by=\"attribute\", attribute=\"role\", value=1)\n\
         raised = poly.extrude_faces@1(inset, cap, offset=[0, 0.2, 0])\n\
         solid = poly.to_solid@1(raised)\n\
         output solid");
    let volume = result.parts[0].volume;
    assert!(
        (volume - (1.0 + 0.8 * 0.8 * 0.2)).abs() < 1.0e-5,
        "volume {volume}"
    );
}

#[test]
fn g6_11_a_selected_edge_is_beveled_with_the_analytic_volume() {
    // The edge along x at y = z = 0.5 (the top front edge of a unit box), chamfered by 0.2.
    let result = ok("box = solid.box@1(size=[1, 1, 1])\n\
         triangles = poly.from_solid@1(box)\n\
         mesh = poly.merge_coplanar@1(triangles)\n\
         edge = poly.select@1(mesh, element=\"edge\", by=\"box\", box_min=[-0.1, 0.4, 0.4], box_max=[0.1, 0.6, 0.6], expect=\"one\")\n\
         beveled = poly.bevel_edges@1(mesh, edge, width=0.2, segments=1)\n\
         solid = poly.to_solid@1(beveled)\n\
         output solid");
    let removed = 1.0 - result.parts[0].volume;
    assert!((removed - 0.02).abs() < 1.0e-5, "removed {removed}");
}

#[test]
fn g6_12_an_open_mesh_renders_with_no_volume_and_takes_a_colour() {
    let result = ok("wall = poly.grid@1(width=4, height=2, columns=4, rows=2)\n\
         paint = solid.material@1(wall, color=[0.8, 0.2, 0.2])\n\
         output paint");
    let part = &result.parts[0];
    assert_eq!(part.indices.len() / 3, 4 * 2 * 2);
    assert!(part.volume.abs() < 1.0e-9, "an open sheet encloses nothing");
    assert!(part.material.as_ref().is_some_and(|m| m.color[0] > 0.7));
}

#[test]
fn g6_facade_every_cell_is_inset_and_its_window_pushed_out() {
    let result = ok("wall = poly.grid@1(width=6, height=4, columns=6, rows=4)\n\
         cells = poly.select@1(wall)\n\
         framed = poly.inset_faces@1(wall, cells, distance=0.15)\n\
         windows = poly.select@1(framed, by=\"attribute\", attribute=\"role\", value=1)\n\
         facade = poly.extrude_faces@1(framed, windows, offset=[0, 0, 0.1], individual=true)\n\
         output facade");
    // 24 cells -> 24 caps + 96 ring quads, then 96 side quads: 24 + 96 + 96 + ... in quads.
    let quads = result.parts[0].indices.len() / 6;
    assert_eq!(quads, 24 + 24 * 4 + 24 * 4);
}

#[test]
fn g6_14_selecting_nothing_many_or_the_wrong_kind_is_refused_with_a_selection_code() {
    let header = "box = solid.box@1(size=[1, 1, 1])\nmesh = poly.from_solid@1(box)\n";
    assert_eq!(
        refused(&format!(
            "{header}none = poly.select@1(mesh, by=\"box\", box_min=[5, 5, 5], box_max=[6, 6, 6])\n\
             gone = poly.delete_faces@1(mesh, none)\noutput gone"
        )),
        "selection.lost"
    );
    assert_eq!(
        refused(&format!(
            "{header}many = poly.select@1(mesh, expect=\"one\")\n\
             gone = poly.delete_faces@1(mesh, many)\noutput gone"
        )),
        "selection.ambiguous"
    );
    assert_eq!(
        refused(&format!(
            "{header}edges = poly.select@1(mesh, element=\"edge\")\n\
             gone = poly.delete_faces@1(mesh, edges)\noutput gone"
        )),
        "poly.selection_kind"
    );
}

#[test]
fn g6_14_a_selection_cannot_be_applied_to_a_different_mesh() {
    // The selection is made on the raw mesh but applied after a dissolve changed the faces.
    let code = refused(
        "box = solid.box@1(size=[1, 1, 1])\n\
         mesh = poly.from_solid@1(box)\n\
         top = poly.select@1(mesh, by=\"normal\", direction=[0, 1, 0], angle=0.05)\n\
         quad = poly.dissolve_faces@1(mesh, top)\n\
         bad = poly.delete_faces@1(quad, top)\n\
         output bad",
    );
    assert_eq!(code, "selection.stale");
}

#[test]
fn g6_operator_refusals_keep_their_poly_codes() {
    // Deleting every face is refused by the operator and surfaces with its stable code.
    let code = refused(
        "box = solid.box@1(size=[1, 1, 1])\n\
         mesh = poly.from_solid@1(box)\n\
         every = poly.select@1(mesh)\n\
         gone = poly.delete_faces@1(mesh, every)\n\
         output gone",
    );
    assert_eq!(code, "poly.delete_all_faces");
}

#[test]
fn g6_09_dissolving_a_triangulated_boxs_diagonal_merges_two_triangles() {
    let result = ok("box = solid.box@1(size=[1, 1, 1])\n\
         mesh = poly.from_solid@1(box)\n\
         diagonal = poly.select@1(mesh, element=\"edge\", by=\"box\", box_min=[-0.05, 0.45, -0.05], box_max=[0.05, 0.55, 0.05], expect=\"one\")\n\
         merged = poly.dissolve_edge@1(mesh, diagonal)\n\
         solid = poly.to_solid@1(merged)\n\
         output solid");
    assert!((result.parts[0].volume - 1.0).abs() < 1.0e-6);
}

#[test]
fn g6_21_dissolving_an_interior_grid_vertex_merges_its_four_cells() {
    let result = ok("wall = poly.grid@1(width=2, height=2, columns=2, rows=2)\n\
         centre = poly.select@1(wall, element=\"vertex\", by=\"box\", box_min=[-0.05, 0.95, -0.05], box_max=[0.05, 1.05, 0.05], expect=\"one\")\n\
         merged = poly.dissolve_vertex@1(wall, centre)\n\
         output merged");
    // One eight-sided face is six triangles.
    assert_eq!(result.parts[0].indices.len() / 3, 6);
}

#[test]
fn g6_06_bridging_the_open_ends_of_two_boxes_makes_a_tunnel_of_exact_volume() {
    // Two unit boxes two metres apart, each with its facing wall deleted, joined by a square tunnel:
    // the enclosed volume is both boxes plus the 1 x 1 x 2 tunnel.
    let result = ok("box = solid.box@1(size=[1, 1, 1])\n\
         other = xform.move@1(box, by=[3, 0, 0])\n\
         both = solid.union@1(box, other)\n\
         triangles = poly.from_solid@1(both)\n\
         mesh = poly.merge_coplanar@1(triangles)\n\
         wall_a = poly.select@1(mesh, by=\"box\", box_min=[0.4, -1, -1], box_max=[0.6, 1, 1], expect=\"one\")\n\
         open_a = poly.delete_faces@1(mesh, wall_a)\n\
         wall_b = poly.select@1(open_a, by=\"box\", box_min=[2.4, -1, -1], box_max=[2.6, 1, 1], expect=\"one\")\n\
         open_b = poly.delete_faces@1(open_a, wall_b)\n\
         ends = poly.select@1(open_b, element=\"loop\")\n\
         bridged = poly.bridge_loops@1(open_b, ends)\n\
         solid = poly.to_solid@1(bridged)\n\
         output solid");
    let volume = result.parts[0].volume;
    assert!((volume - 4.0).abs() < 1.0e-4, "volume {volume}");
}

#[test]
fn g6_edits_are_cached_and_repeatable() {
    let text = "wall = poly.grid@1(width=2, height=2, columns=2, rows=2)\n\
                cells = poly.select@1(wall)\n\
                framed = poly.inset_faces@1(wall, cells, distance=0.1)\n\
                output framed";
    let first = ok(text);
    let second = ok(text);
    assert_eq!(first.parts[0].positions, second.parts[0].positions);
    assert_eq!(first.parts[0].indices, second.parts[0].indices);
}
