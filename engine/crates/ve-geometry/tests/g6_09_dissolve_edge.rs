//! Acceptance tests for transactional edge dissolution.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup and assertions should stop at their first failure"
)]

use ve_geometry::{
    AttrData, Attribute, Domain, Element, GeometryError, PolyMesh, Topology, dissolve_edge,
    validate,
};
use ve_kernel::ExecutionContext;

fn context() -> ExecutionContext {
    ExecutionContext::new().unwrap()
}

fn mesh(positions: Vec<[f32; 3]>, faces: &[&[u32]]) -> PolyMesh {
    let sizes = faces
        .iter()
        .map(|face| u32::try_from(face.len()).unwrap())
        .collect();
    let corners = faces.iter().flat_map(|face| face.iter().copied()).collect();
    PolyMesh::new(positions, sizes, corners).unwrap()
}

fn split_quad() -> PolyMesh {
    mesh(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ],
        &[&[0, 1, 2], &[0, 2, 3]],
    )
}

#[test]
fn g6_09_dissolve_edge_merges_two_triangles_and_preserves_corner_attributes() {
    let mut source = split_quad();
    source
        .set_attribute(
            "face_tag",
            Attribute {
                domain: Domain::Face,
                data: AttrData::U32(vec![8, 8]),
            },
        )
        .unwrap();
    source
        .set_attribute(
            "uv_corner_id",
            Attribute {
                domain: Domain::Corner,
                data: AttrData::U32(vec![10, 11, 12, 20, 21, 22]),
            },
        )
        .unwrap();
    let before = validate(&source, &context()).unwrap();
    let merged = dissolve_edge(&source, (0, 2), &context()).unwrap();
    let after = validate(&merged, &context()).unwrap();
    assert_eq!(merged.face_sizes(), [4]);
    assert_eq!(merged.corners(), [0, 1, 2, 3]);
    assert_eq!(merged.attributes()["face_tag"].data, AttrData::U32(vec![8]));
    assert_eq!(
        merged.attributes()["uv_corner_id"].data,
        AttrData::U32(vec![10, 11, 12, 22])
    );
    assert_eq!(before.euler_characteristic, after.euler_characteristic);
    assert_eq!(after.edges, 4);
    assert_eq!(after.components, 1);
}

#[test]
fn g6_09_dissolve_edge_names_boundary_nonmanifold_and_attribute_refusals() {
    let source = split_quad();
    let before = source.clone();
    assert!(matches!(
        dissolve_edge(&source, (0, 1), &context()),
        Err(GeometryError::PolyRefusal {
            code: "poly.boundary_edge",
            element: Some(Element::Edge(0, 1)),
        })
    ));
    assert_eq!(source, before);

    let three_face_edge = mesh(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.5, -1.0, 0.0],
        ],
        &[&[0, 1, 2], &[1, 0, 3], &[0, 1, 4]],
    );
    assert!(matches!(
        dissolve_edge(&three_face_edge, (1, 0), &context()),
        Err(GeometryError::PolyRefusal {
            code: "poly.nonmanifold",
            element: Some(Element::Edge(0, 1)),
        })
    ));

    let mut conflicting = split_quad();
    conflicting
        .set_attribute(
            "face_tag",
            Attribute {
                domain: Domain::Face,
                data: AttrData::U32(vec![1, 2]),
            },
        )
        .unwrap();
    assert!(matches!(
        dissolve_edge(&conflicting, (0, 2), &context()),
        Err(GeometryError::PolyRefusal {
            code: "poly.attribute_conflict",
            element: Some(Element::Edge(0, 2)),
        })
    ));
}

#[test]
fn g6_09_dissolve_edge_preserves_unrelated_sliver_faces() {
    let source = mesh(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0e-8, 0.0],
            [0.0, 0.0, 1.0],
            [3.0, 0.0, 0.0],
            [4.0, 0.0, 0.0],
            [4.0, 1.0, 0.0],
            [3.0, 1.0, 0.0],
        ],
        &[
            &[0, 2, 1],
            &[0, 1, 3],
            &[1, 2, 3],
            &[2, 0, 3],
            &[4, 5, 6],
            &[4, 6, 7],
        ],
    );
    let result = dissolve_edge(&source, (4, 6), &context()).unwrap();
    assert_eq!(
        validate(&result, &context()).unwrap().degenerate_faces,
        [0, 2]
    );
}

#[test]
fn g6_16_dissolve_edge_large_grid_and_cancellation() {
    let side = 224_u32;
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
    let mut sizes = Vec::new();
    let mut corners = Vec::new();
    for y in 0..side {
        for x in 0..side {
            let a = y * (side + 1) + x;
            let b = a + 1;
            let d = a + side + 1;
            let c = d + 1;
            sizes.extend([3, 3]);
            corners.extend([a, b, c, a, c, d]);
        }
    }
    let large = PolyMesh::new(positions, sizes, corners).unwrap();
    let result = dissolve_edge(&large, (0, 226), &context()).unwrap();
    assert_eq!(result.face_count(), 100_351);
    assert_eq!(
        Topology::build(&result, &context()).unwrap().edges().len(),
        150_975
    );

    let context = context();
    context.cancel();
    assert!(matches!(
        dissolve_edge(&split_quad(), (0, 2), &context),
        Err(GeometryError::Cancelled)
    ));
}
