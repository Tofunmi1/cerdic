//! Regression fixtures captured from our own lamp and goblet procedural outputs.
#![expect(
    clippy::unwrap_used,
    reason = "fixture parsing failures should stop the acceptance test immediately"
)]

use ve_geometry::{
    Element, GeometryError, PolyMesh, Topology, delete_faces, dissolve_edge, dissolve_faces,
    dissolve_vertex, extrude_faces, flip_faces, inset_face, validate, weld,
};
use ve_kernel::ExecutionContext;

fn context() -> ExecutionContext {
    ExecutionContext::new().unwrap()
}

fn fixture(text: &str) -> PolyMesh {
    let mut tokens = text.split_whitespace();
    assert_eq!(tokens.next(), Some("VE_POLY_FIXTURE"));
    assert_eq!(tokens.next(), Some("1"));
    assert_eq!(tokens.next(), Some("positions"));
    let point_count: usize = tokens.next().unwrap().parse().unwrap();
    let mut positions = Vec::with_capacity(point_count);
    for _ in 0..point_count {
        assert_eq!(tokens.next(), Some("v"));
        positions.push([
            tokens.next().unwrap().parse::<f32>().unwrap(),
            tokens.next().unwrap().parse::<f32>().unwrap(),
            tokens.next().unwrap().parse::<f32>().unwrap(),
        ]);
    }
    assert_eq!(tokens.next(), Some("triangles"));
    let face_count: usize = tokens.next().unwrap().parse().unwrap();
    let mut sizes = Vec::with_capacity(face_count);
    let mut corners = Vec::with_capacity(face_count * 3);
    for _ in 0..face_count {
        assert_eq!(tokens.next(), Some("t"));
        sizes.push(3);
        corners.extend([
            tokens.next().unwrap().parse::<u32>().unwrap(),
            tokens.next().unwrap().parse::<u32>().unwrap(),
            tokens.next().unwrap().parse::<u32>().unwrap(),
        ]);
    }
    assert_eq!(tokens.next(), None);
    PolyMesh::new(positions, sizes, corners).unwrap()
}

#[test]
fn g6_01_lamp_shade_and_goblet_keep_their_reported_sliver_faces() {
    exercise_sliver_fixture(include_str!("fixtures/lamp_shade.poly"));
    exercise_sliver_fixture(include_str!("fixtures/goblet.poly"));
}

fn exercise_sliver_fixture(source: &str) {
    let mesh = fixture(source);
    let context = context();
    let report = validate(&mesh, &context).unwrap();
    let sliver = *report.degenerate_faces.first().unwrap();
    let topology = Topology::build(&mesh, &context).unwrap();
    assert_sliver_selections_refuse(&mesh, sliver, &topology, &context);
    assert_edits_away_succeed(&mesh, &report.degenerate_faces, &topology, &context);
}

fn assert_sliver_selections_refuse(
    mesh: &PolyMesh,
    sliver: usize,
    topology: &Topology,
    context: &ExecutionContext,
) {
    assert!(matches!(
        flip_faces(mesh, &[sliver], true, context),
        Err(GeometryError::PolyRefusal {
            code: "poly.face_degenerate",
            element: Some(Element::Face(face)),
        }) if face == sliver
    ));
    assert!(matches!(
        extrude_faces(mesh, &[sliver], [0.0, 0.0, 0.1], context),
        Err(GeometryError::PolyRefusal {
            code: "poly.face_degenerate",
            element: Some(Element::Face(face)),
        }) if face == sliver
    ));
    assert!(matches!(
        inset_face(mesh, sliver, 1.0e-4, context),
        Err(GeometryError::PolyRefusal {
            code: "poly.face_degenerate",
            element: Some(Element::Face(face)),
        }) if face == sliver
    ));
    assert!(matches!(
        delete_faces(mesh, &[sliver], context),
        Err(GeometryError::PolyRefusal {
            code: "poly.face_degenerate",
            element: Some(Element::Face(face)),
        }) if face == sliver
    ));
    assert!(matches!(
        dissolve_faces(mesh, &[sliver], context),
        Err(GeometryError::PolyRefusal {
            code: "poly.face_degenerate",
            element: Some(Element::Face(face)),
        }) if face == sliver
    ));
    let sliver_corner = mesh.face_sizes()[..sliver]
        .iter()
        .map(|size| usize::try_from(*size).unwrap())
        .sum::<usize>();
    let sliver_vertex = mesh.corners()[sliver_corner];
    let sliver_vertex_result = dissolve_vertex(mesh, sliver_vertex, context);
    assert!(
        matches!(
            sliver_vertex_result,
            Err(GeometryError::PolyRefusal {
                code: "poly.face_degenerate",
                ..
            })
        ),
        "sliver vertex {sliver_vertex}: {sliver_vertex_result:?}"
    );
    let sliver_edge = topology
        .edges()
        .iter()
        .find(|(_, info)| {
            info.face_count() == 2 && info.uses().iter().any(|usage| usage.face == sliver)
        })
        .map(|(&edge, _)| edge)
        .unwrap();
    assert!(matches!(
        dissolve_edge(mesh, sliver_edge, context),
        Err(GeometryError::PolyRefusal {
            code: "poly.face_degenerate",
            ..
        })
    ));
}

fn assert_edits_away_succeed(
    mesh: &PolyMesh,
    slivers: &[usize],
    topology: &Topology,
    context: &ExecutionContext,
) {
    let weld_result = weld(mesh, 1.0e-12, 10_000_000, context);
    assert!(
        matches!(
            &weld_result,
            Ok(_)
                | Err(GeometryError::PolyRefusal {
                    code: "poly.weld_collapses_face" | "poly.face_degenerate",
                    element: Some(Element::Face(_)),
                })
        ),
        "{weld_result:?}"
    );
    if let Ok(result) = weld_result {
        assert!(!result.mesh.face_sizes().is_empty());
    }
    let editable_face = (0..mesh.face_count())
        .find(|face| !slivers.contains(face))
        .unwrap();
    assert!(flip_faces(mesh, &[editable_face], true, context).is_ok());
    let safe_edge = topology
        .edges()
        .iter()
        .find(|(_, info)| {
            info.face_count() == 2
                && info
                    .uses()
                    .iter()
                    .all(|usage| !slivers.contains(&usage.face))
        })
        .map(|(&edge, _)| edge)
        .unwrap();
    assert!(dissolve_edge(mesh, safe_edge, context).is_ok());
    let safe_patch = topology
        .edges()
        .values()
        .find(|info| {
            info.face_count() == 2
                && info
                    .uses()
                    .iter()
                    .all(|usage| !slivers.contains(&usage.face))
        })
        .unwrap()
        .uses()
        .iter()
        .map(|usage| usage.face)
        .collect::<Vec<_>>();
    assert!(dissolve_faces(mesh, &safe_patch, context).is_ok());
    let vertex_edit_succeeded = (0..mesh.positions().len())
        .map(|index| u32::try_from(index).unwrap())
        .filter(|vertex| {
            let faces = &topology.vertex_faces()[usize::try_from(*vertex).unwrap()];
            !faces.is_empty() && faces.iter().all(|face| !slivers.contains(face))
        })
        .take(32)
        .any(|vertex| dissolve_vertex(mesh, vertex, context).is_ok());
    assert!(
        vertex_edit_succeeded,
        "no sliver-free vertex fan could be dissolved"
    );
}
