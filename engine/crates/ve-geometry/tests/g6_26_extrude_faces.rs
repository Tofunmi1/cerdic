//! Acceptance tests for connected planar face extrusion.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup and assertions should stop at their first failure"
)]

use ve_geometry::{
    AttrData, Attribute, Domain, Element, GeometryError, PolyMesh, extrude_faces, validate,
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

fn quad_grid(side: u32) -> PolyMesh {
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
fn g6_26_extrudes_a_closed_cube_face_and_preserves_attribute_provenance() {
    let mut source = cube();
    source
        .set_attribute(
            "point_id",
            Attribute {
                domain: Domain::Point,
                data: AttrData::U32((0..8).collect()),
            },
        )
        .unwrap();
    source
        .set_attribute(
            "face_id",
            Attribute {
                domain: Domain::Face,
                data: AttrData::U32((10..16).collect()),
            },
        )
        .unwrap();
    source
        .set_attribute(
            "corner_id",
            Attribute {
                domain: Domain::Corner,
                data: AttrData::U32((0..24).collect()),
            },
        )
        .unwrap();
    let result = extrude_faces(&source, &[1], [0.0, 0.0, 1.0], &context()).unwrap();
    let report = validate(&result, &context()).unwrap();
    assert_eq!(result.face_count(), 10);
    assert_eq!(report.euler_characteristic, 2);
    assert!(report.nonmanifold_edges.is_empty());
    assert!(report.inconsistent_winding_edges.is_empty());
    assert_eq!(
        result.attributes()["point_id"].data,
        AttrData::U32(vec![0, 1, 2, 3, 4, 5, 6, 7, 4, 5, 6, 7])
    );
    assert_eq!(
        result.attributes()["face_id"].data,
        AttrData::U32(vec![10, 11, 12, 13, 14, 15, 11, 11, 11, 11])
    );
    assert_eq!(result.attributes()["corner_id"].data.len(), 40);
}

#[test]
fn g6_27_extrusion_refuses_bad_direction() {
    let source = cube();
    assert!(matches!(
        extrude_faces(&source, &[1], [1.0, 0.0, 0.0], &context()),
        Err(GeometryError::PolyRefusal {
            code: "poly.direction_invalid",
            element: Some(Element::Face(1))
        })
    ));
    assert!(matches!(
        extrude_faces(&source, &[1], [0.0, 0.0, 0.0], &context()),
        Err(GeometryError::PolyRefusal {
            code: "poly.direction_invalid",
            ..
        })
    ));
    assert!(matches!(
        extrude_faces(&source, &[1], [0.0, 0.0, -1.0], &context()),
        Err(GeometryError::PolyRefusal {
            code: "poly.direction_invalid",
            ..
        })
    ));
    assert!(matches!(
        extrude_faces(&source, &[1], [f32::NAN, 0.0, 1.0], &context()),
        Err(GeometryError::PolyRefusal {
            code: "poly.direction_invalid",
            ..
        })
    ));
}

#[test]
fn g6_28_extrusion_observes_cancellation_and_scales_to_large_input() {
    let small = cube();
    let cancelled = context();
    cancelled.cancel();
    assert!(matches!(
        extrude_faces(&small, &[1], [0.0, 0.0, 1.0], &cancelled),
        Err(GeometryError::Cancelled)
    ));

    let large = quad_grid(317);
    assert_eq!(large.face_count(), 100_489);
    let result = extrude_faces(&large, &[50_000], [0.0, 0.0, 0.25], &context()).unwrap();
    assert_eq!(result.face_count(), large.face_count() + 4);
}
