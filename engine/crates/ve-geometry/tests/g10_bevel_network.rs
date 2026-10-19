//! `bevel_network`: attributes follow the elements they descend from, and bad selections refuse.
#![expect(
    clippy::unwrap_used,
    reason = "tests assert by unwrapping fixtures that are known to be valid"
)]

use ve_geometry::{
    AttrData, Attribute, Domain, GeometryError, NetworkBevel, NetworkBevelOffset, PolyMesh,
    TriangulationMode, bevel_network,
};
use ve_kernel::ExecutionContext;

fn cube() -> PolyMesh {
    PolyMesh::new(
        vec![
            [-1.0, -1.0, -1.0],
            [1.0, -1.0, -1.0],
            [1.0, 1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, -1.0, 1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, 1.0],
            [-1.0, 1.0, 1.0],
        ],
        vec![4; 6],
        vec![
            0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6, 3, 0, 4, 7,
        ],
    )
    .unwrap()
}

fn options(width: f64, segments: usize) -> NetworkBevel {
    NetworkBevel {
        width,
        segments,
        profile: 0.5,
        offset: NetworkBevelOffset::Offset,
    }
}

#[test]
fn face_point_and_corner_attributes_follow_their_sources() {
    let mut mesh = cube();
    mesh.set_attribute(
        "face_id",
        Attribute {
            domain: Domain::Face,
            data: AttrData::U32((0..6).collect()),
        },
    )
    .unwrap();
    mesh.set_attribute(
        "point_id",
        Attribute {
            domain: Domain::Point,
            data: AttrData::U32((0..8).collect()),
        },
    )
    .unwrap();
    mesh.set_attribute(
        "corner_id",
        Attribute {
            domain: Domain::Corner,
            data: AttrData::U32((0..24).collect()),
        },
    )
    .unwrap();
    let context = ExecutionContext::new().unwrap();
    let result = bevel_network(&mesh, &[(4, 5)], &options(0.25, 3), &context).unwrap();
    assert_eq!(result.triangulation_mode(), TriangulationMode::CurvedQuads);
    let AttrData::U32(faces) = &result.attributes()["face_id"].data else {
        unreachable!("face_id stays unsigned")
    };
    assert_eq!(faces.len(), result.face_count());
    assert!(faces.iter().all(|&f| f < 6));
    for face in 0..6 {
        assert!(faces.contains(&face), "input face {face} lost");
    }
    let AttrData::U32(points) = &result.attributes()["point_id"].data else {
        unreachable!("point_id stays unsigned")
    };
    assert_eq!(points.len(), result.positions().len());
    assert!(points.iter().all(|&p| p < 8));
    let AttrData::U32(corners) = &result.attributes()["corner_id"].data else {
        unreachable!("corner_id stays unsigned")
    };
    assert_eq!(corners.len(), result.corners().len());
    assert!(corners.iter().all(|&c| c < 24));
}

#[test]
fn edge_attributes_refuse_because_new_edges_have_no_source() {
    let mut mesh = cube();
    let edges = mesh.edge_count();
    mesh.set_attribute(
        "crease",
        Attribute {
            domain: Domain::Edge,
            data: AttrData::F32(vec![0.0; edges]),
        },
    )
    .unwrap();
    let context = ExecutionContext::new().unwrap();
    let error = bevel_network(&mesh, &[(4, 5)], &options(0.25, 3), &context).unwrap_err();
    assert!(matches!(
        error,
        GeometryError::PolyRefusal {
            code: "poly.attribute_conflict",
            ..
        }
    ));
}

#[test]
fn bad_selections_and_options_refuse_with_typed_codes() {
    let mesh = cube();
    let context = ExecutionContext::new().unwrap();
    let code = |result: Result<PolyMesh, GeometryError>| match result.unwrap_err() {
        GeometryError::PolyRefusal { code, .. } => code,
        other => panic!("not a refusal: {other}"),
    };
    assert_eq!(
        code(bevel_network(&mesh, &[], &options(0.1, 2), &context)),
        "poly.selection_empty"
    );
    assert_eq!(
        code(bevel_network(&mesh, &[(0, 6)], &options(0.1, 2), &context)),
        "poly.selection_invalid"
    );
    assert_eq!(
        code(bevel_network(&mesh, &[(0, 3)], &options(-1.0, 2), &context)),
        "poly.bevel_width_invalid"
    );
    assert_eq!(
        code(bevel_network(&mesh, &[(0, 3)], &options(0.1, 17), &context)),
        "poly.bevel_segments_invalid"
    );
    let mut flat = options(0.1, 2);
    flat.profile = 0.0;
    assert_eq!(
        code(bevel_network(&mesh, &[(0, 3)], &flat, &context)),
        "poly.bevel_profile_invalid"
    );
    // An open quad: its edges have one face.
    let quad = PolyMesh::new(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ],
        vec![4],
        vec![0, 1, 2, 3],
    )
    .unwrap();
    assert_eq!(
        code(bevel_network(&quad, &[(0, 1)], &options(0.1, 2), &context)),
        "poly.boundary_edge"
    );
}

#[test]
fn a_cancelled_context_stops_before_blender_runs() {
    // Blender's bevel has no cancellation points of its own; the operator checks the context before
    // and after it (a bevel of 540,000 faces takes about 0.3 s natively), so a cancelled evaluation
    // never starts one and never returns its result.
    let mesh = cube();
    let context = ExecutionContext::new().unwrap();
    context.cancel();
    let error = bevel_network(&mesh, &[(4, 5)], &options(0.25, 3), &context).unwrap_err();
    assert_eq!(error, GeometryError::Cancelled);
}

#[test]
fn the_input_is_never_changed() {
    let mesh = cube();
    let before = mesh.clone();
    let context = ExecutionContext::new().unwrap();
    let _ = bevel_network(&mesh, &[(4, 5), (5, 6)], &options(0.2, 2), &context).unwrap();
    assert_eq!(mesh, before);
}
