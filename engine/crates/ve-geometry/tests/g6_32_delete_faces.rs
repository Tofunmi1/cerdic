//! Acceptance tests for deleting faces while retaining a valid open surface.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup and assertions should stop at their first failure"
)]

use ve_geometry::{
    AttrData, Attribute, Domain, Element, GeometryError, PolyMesh, delete_faces, validate,
};
use ve_kernel::ExecutionContext;

fn context() -> ExecutionContext {
    ExecutionContext::new().unwrap()
}

fn cube() -> PolyMesh {
    let positions = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
        [0.0, 1.0, 1.0],
    ];
    let corners = vec![
        0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 3, 7, 6, 2, 0, 4, 7, 3, 1, 2, 6, 5,
    ];
    PolyMesh::new(positions, vec![4; 6], corners).unwrap()
}

fn grid(side: u32) -> PolyMesh {
    let mut positions = Vec::new();
    for y in 0..=side {
        for x in 0..=side {
            positions.push([
                f32::from(u16::try_from(x).unwrap()),
                f32::from(u16::try_from(y).unwrap()),
                0.0,
            ]);
        }
    }
    let mut corners = Vec::new();
    for y in 0..side {
        for x in 0..side {
            let a = y * (side + 1) + x;
            let b = a + 1;
            let d = a + side + 1;
            corners.extend([a, b, d + 1, d]);
        }
    }
    PolyMesh::new(positions, vec![4; (side * side) as usize], corners).unwrap()
}

#[test]
fn g6_32_delete_faces_opens_a_cube_and_remaps_attributes() {
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
        "corner_id",
        Attribute {
            domain: Domain::Corner,
            data: AttrData::U32((0..24).collect()),
        },
    )
    .unwrap();
    let result = delete_faces(&mesh, &[1], &context()).unwrap();
    let report = validate(&result, &context()).unwrap();
    assert_eq!(result.face_count(), 5);
    assert_eq!(report.euler_characteristic, 1);
    assert_eq!(report.boundary_loops, 1);
    assert_eq!(
        result.attributes()["face_id"].data,
        AttrData::U32(vec![0, 2, 3, 4, 5])
    );
    assert_eq!(result.attributes()["corner_id"].data.len(), 20);
}

#[test]
fn g6_33_delete_faces_refuses_isolated_vertices_and_empty_outputs() {
    let tetra = PolyMesh::new(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
        ],
        vec![3; 4],
        vec![0, 2, 1, 0, 1, 3, 1, 2, 3, 2, 0, 3],
    )
    .unwrap();
    assert!(matches!(
        delete_faces(&tetra, &[0, 1, 2], &context()),
        Err(GeometryError::PolyRefusal {
            code: "poly.vertex_isolated",
            element: Some(Element::Vertex(_))
        })
    ));
    assert!(matches!(
        delete_faces(&tetra, &[0, 1, 2, 3], &context()),
        Err(GeometryError::PolyRefusal {
            code: "poly.delete_all_faces",
            ..
        })
    ));
}

#[test]
fn g6_16_delete_faces_scales_and_observes_cancellation() {
    let large = grid(317);
    assert_eq!(large.face_count(), 100_489);
    let result = delete_faces(&large, &[50_000], &context()).unwrap();
    assert_eq!(result.face_count(), large.face_count() - 1);
    let cancelled = context();
    cancelled.cancel();
    assert!(matches!(
        delete_faces(&cube(), &[1], &cancelled),
        Err(GeometryError::Cancelled)
    ));
}
