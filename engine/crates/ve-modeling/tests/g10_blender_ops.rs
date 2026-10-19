//! Blender's mesh operators through the node language: inset, extrude, solidify, poke, subdivide,
//! dissolve, wireframe and symmetrize, checked against closed-form volumes.
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
        transaction: TransactionId::parse("t_g10_blender_ops").unwrap(),
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

fn volume(text: &str) -> f64 {
    evaluate(text)
        .unwrap_or_else(|(code, message)| panic!("{code}: {message}"))
        .parts[0]
        .volume
}

fn refused(text: &str) -> String {
    evaluate(text).err().map(|(code, _)| code).unwrap()
}

const BOX: &str = "box = solid.box@1(size=[1, 1, 1])\n\
    triangles = poly.from_solid@1(box)\n\
    mesh = poly.merge_coplanar@1(triangles)\n";
const TOP: &str = "top = poly.select@1(mesh, by=\"normal\", direction=[0, 1, 0], angle=0.05)\n";

#[test]
fn a_raised_panel_is_an_inset_then_an_extrusion() {
    // Inset the top by 0.1 (the inner face is 0.8 square), pick it by its centre, push it up 0.3.
    let v = volume(&format!(
        "{BOX}{TOP}\
         inset = poly.inset_region@1(mesh, top, thickness=0.1)\n\
         inner = poly.select@1(inset, by=\"box\", box_min=[-0.2, 0.4, -0.2], box_max=[0.2, 0.6, 0.2])\n\
         raised = poly.extrude_region@1(inset, inner, offset=[0, 0.3, 0])\n\
         solid = poly.to_solid@1(raised)\n\
         output solid"
    ));
    assert!((v - (1.0 + 0.64 * 0.3)).abs() < 1e-4, "volume {v}");
}

#[test]
fn inset_and_extrude_tag_their_faces_with_role_so_no_box_is_needed() {
    // Inset tags the inner face 1 and the ring 2; extrude tags the moved cap 1 and the walls 2. Picking
    // by role needs no coordinates: a stepped tower of two 0.64 m^2 slabs, 0.3 m then 0.2 m.
    let v = volume(&format!(
        "{BOX}{TOP}\
         inset = poly.inset_region@1(mesh, top, thickness=0.1)\n\
         inner = poly.select@1(inset, by=\"attribute\", attribute=\"role\", value=1)\n\
         step1 = poly.extrude_region@1(inset, inner, offset=[0, 0.3, 0])\n\
         cap = poly.select@1(step1, by=\"attribute\", attribute=\"role\", value=1)\n\
         step2 = poly.extrude_region@1(step1, cap, offset=[0, 0.2, 0])\n\
         solid = poly.to_solid@1(step2)\n\
         output solid"
    ));
    assert!(
        (v - (1.0 + 0.64 * 0.3 + 0.64 * 0.2)).abs() < 1e-4,
        "volume {v}"
    );
}

#[test]
fn poke_raises_a_pyramid_on_the_selected_face() {
    let v = volume(&format!(
        "{BOX}{TOP}\
         roof = poly.poke@1(mesh, top, offset=0.5)\n\
         solid = poly.to_solid@1(roof)\n\
         output solid"
    ));
    assert!((v - (1.0 + 0.5 / 3.0)).abs() < 1e-4, "volume {v}");
}

#[test]
fn solidify_turns_an_open_grid_into_a_slab() {
    let v = volume(
        "wall = poly.grid@1(width=4, height=2, columns=4, rows=2)\n\
         slab = poly.solidify@1(wall, thickness=0.1)\n\
         solid = poly.to_solid@1(slab)\n\
         output solid",
    );
    assert!((v - 0.8).abs() < 1e-4, "volume {v}");
}

#[test]
fn symmetrize_rebuilds_a_box_from_its_positive_half() {
    let v = volume(&format!(
        "{BOX}whole = poly.symmetrize@1(mesh, axis=\"x\", keep=\"positive\")\n\
         solid = poly.to_solid@1(whole)\n\
         output solid"
    ));
    assert!((v - 1.0).abs() < 1e-4, "volume {v}");
}

#[test]
fn dissolve_limit_and_subdivide_keep_the_volume() {
    let dissolved = volume(&format!(
        "{BOX}clean = poly.dissolve_limit@1(triangles, angle=0.1)\n\
         solid = poly.to_solid@1(clean)\n\
         output solid"
    ));
    assert!((dissolved - 1.0).abs() < 1e-4, "volume {dissolved}");
    let cut = volume(&format!(
        "{BOX}edge = poly.select@1(mesh, element=\"edge\", by=\"box\", box_min=[-0.1, 0.4, 0.4], box_max=[0.1, 0.6, 0.6], expect=\"one\")\n\
         more = poly.subdivide_edges@1(mesh, edge, cuts=2)\n\
         solid = poly.to_solid@1(more)\n\
         output solid"
    ));
    assert!((cut - 1.0).abs() < 1e-4, "volume {cut}");
}

#[test]
fn unwrapping_a_box_along_its_sharp_edges_keeps_the_shape() {
    // The UVs live in corner attributes; the shape is unchanged and the node refuses bad input.
    let v = volume(&format!(
        "{BOX}seams = poly.select@1(mesh, element=\"edge\", by=\"sharp\", angle=0.5)\n\
         uv = poly.unwrap_uv@1(mesh, seams, method=\"angle\", rotate=true)\n\
         solid = poly.to_solid@1(uv)\n\
         output solid"
    ));
    assert!((v - 1.0).abs() < 1e-4, "volume {v}");
    let faces = refused(&format!(
        "{BOX}{TOP}\
         uv = poly.unwrap_uv@1(mesh, top)\n\
         solid = poly.to_solid@1(uv)\n\
         output solid"
    ));
    assert_eq!(faces, "poly.selection_kind");
}

#[test]
fn blender_uvs_survive_texture_assignment_and_reach_the_evaluated_part() {
    let text = "box = solid.box@1(size=[1, 1, 1])\n\
        triangles = poly.from_solid@1(box)\n\
        cage = poly.merge_coplanar@1(triangles)\n\
        seams = poly.select@1(cage, element=\"edge\", by=\"sharp\", angle=0.5)\n\
        uv = poly.unwrap_uv@1(cage, seams, method=\"angle\", rotate=true)\n\
        solid = poly.to_solid@1(uv)\n\
        painted = solid.texture@1(solid, program=\"grain = fbm(scale_x=5); albedo = grain\")\n\
        output painted";
    let result = evaluate(text).unwrap_or_else(|(code, message)| panic!("{code}: {message}"));
    let part = &result.parts[0];
    assert_eq!(
        part.uvs.len(),
        part.indices.len(),
        "one UV per triangle corner"
    );
    assert!(part.uvs.iter().flatten().all(|uv| (0.0..=1.0).contains(uv)));
    let seam_vertex_keeps_multiple_uvs = (0..part.positions.len()).any(|vertex| {
        let values: std::collections::BTreeSet<_> = part
            .indices
            .iter()
            .enumerate()
            .filter(|(_, index)| **index as usize == vertex)
            .map(|(corner, _)| part.uvs[corner].map(f32::to_bits))
            .collect();
        values.len() > 1
    });
    assert!(
        seam_vertex_keeps_multiple_uvs,
        "UV seams remain split per corner"
    );
    assert!(
        part.material
            .as_ref()
            .is_some_and(|material| material.texture_program.is_some())
    );
}

#[test]
fn unwrap_cuts_seams_by_itself_and_refuses_a_closed_surface_without_them() {
    // A cube with no seams selected: the sharp edges are cut automatically and the layout is clean.
    let cube = "box = solid.box@1(size=[1, 1, 1])\n\
        tris = poly.from_solid@1(box)\n\
        cage = poly.merge_coplanar@1(tris)\n\
        uv = poly.unwrap_uv@1(cage)\n\
        solid = poly.to_solid@1(uv)\n\
        output solid";
    let result = evaluate(cube).unwrap_or_else(|(code, message)| panic!("{code}: {message}"));
    assert!(
        result
            .warnings
            .iter()
            .all(|w| !w.code.starts_with("poly.uv.")),
        "a cube unwraps cleanly: {:?}",
        result.warnings
    );
    assert_eq!(result.parts[0].uvs.len(), result.parts[0].indices.len());
    // A smooth sphere is cut into near-flat charts and unwraps cleanly.
    let sphere = "ball = solid.sphere@1(radius=1, segments=24)\n\
        tris = poly.from_solid@1(ball)\n\
        uv = poly.unwrap_uv@1(tris)\n\
        solid = poly.to_solid@1(uv)\n\
        output solid";
    let result = evaluate(sphere).unwrap_or_else(|(code, message)| panic!("{code}: {message}"));
    assert!(
        result
            .warnings
            .iter()
            .all(|w| !w.code.starts_with("poly.uv.")),
        "a sphere unwraps cleanly: {:?}",
        result.warnings
    );
    // (The stretch warning itself is driven by `uv_quality`, whose numbers are asserted in ve-geometry's
    // g10_auto_seams test; the modeling shapes here are too well behaved to trip it.)
    // Turning automatic seams off on a closed surface gives nothing usable, and the refusal says why.
    let refused = refused(&format!(
        "ball = solid.sphere@1(radius=1, segments=24)\n\
         tris = poly.from_solid@1(ball)\n\
         uv = poly.unwrap_uv@1(tris, auto_seams=false)\n\
         solid = poly.to_solid@1(uv)\n\
         output solid"
    ));
    assert_eq!(refused, "poly.uv_unwrap_failed");
}

#[test]
fn curved_shapes_unwrap_by_themselves_without_a_refusal_or_a_distortion_warning() {
    // A torus has a handle, a capped can mixes flat caps with a curved wall, a sphere is closed and smooth:
    // none needs a selected seam, and none may come out stretched, flipped or overlapping badly enough to warn.
    for (name, shape) in [
        (
            "torus",
            "shape = solid.torus@1(major=0.5, minor=0.15, segments=32, tube_segments=16)",
        ),
        (
            "can",
            "shape = solid.cylinder@1(height=1, radius=0.4, segments=24)",
        ),
        ("ball", "shape = solid.sphere@1(radius=0.5, segments=24)"),
    ] {
        let program = format!(
            "{shape}\ntris = poly.from_solid@1(shape)\nuv = poly.unwrap_uv@1(tris)\nsolid = poly.to_solid@1(uv)\noutput solid"
        );
        let result = evaluate(&program)
            .unwrap_or_else(|(code, message)| panic!("{name}: {code}: {message}"));
        let part = &result.parts[0];
        assert_eq!(
            part.uvs.len(),
            part.indices.len(),
            "{name}: a UV per corner"
        );
        assert!(
            part.uvs
                .iter()
                .flatten()
                .all(|uv| (-1e-4..=1.0 + 1e-4).contains(uv)),
            "{name}: in the unit square"
        );
        assert!(
            result
                .warnings
                .iter()
                .all(|w| !w.code.starts_with("poly.uv.")),
            "{name}: {:?}",
            result.warnings
        );
    }
}

include!("g10_blender_ops/part_1.rs");
include!("g10_blender_ops/part_2.rs");
