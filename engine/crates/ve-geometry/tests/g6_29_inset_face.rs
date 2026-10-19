//! Acceptance tests for single-face convex insets.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup and assertions should stop at their first failure"
)]

use ve_geometry::{AttrData, Attribute, Domain, GeometryError, PolyMesh, inset_face, validate};
use ve_kernel::ExecutionContext;

fn context() -> ExecutionContext {
    ExecutionContext::new().unwrap()
}

fn square() -> PolyMesh {
    PolyMesh::new(
        vec![
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 2.0, 0.0],
            [0.0, 2.0, 0.0],
        ],
        vec![4],
        vec![0, 1, 2, 3],
    )
    .unwrap()
}

fn grid(side: u32) -> PolyMesh {
    let mut points = Vec::new();
    for y in 0..=side {
        for x in 0..=side {
            points.push([
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
    PolyMesh::new(points, vec![4; (side * side) as usize], corners).unwrap()
}

#[test]
fn g6_29_insets_convex_quad_and_preserves_data_provenance() {
    let mut mesh = square();
    mesh.set_attribute(
        "face_tag",
        Attribute {
            domain: Domain::Face,
            data: AttrData::U32(vec![17]),
        },
    )
    .unwrap();
    mesh.set_attribute(
        "corner_tag",
        Attribute {
            domain: Domain::Corner,
            data: AttrData::U32(vec![0, 1, 2, 3]),
        },
    )
    .unwrap();
    mesh.set_attribute(
        "point_tag",
        Attribute {
            domain: Domain::Point,
            data: AttrData::U32(vec![10, 11, 12, 13]),
        },
    )
    .unwrap();
    let inset = inset_face(&mesh, 0, 0.25, &context()).unwrap();
    assert_eq!(inset.face_count(), 5);
    assert_eq!(inset.face_sizes(), [4, 4, 4, 4, 4]);
    assert_eq!(inset.positions().len(), 8);
    assert_eq!(
        inset.attributes()["face_tag"].data,
        AttrData::U32(vec![17; 5])
    );
    assert_eq!(inset.attributes()["corner_tag"].data.len(), 20);
    assert_eq!(
        inset.attributes()["point_tag"].data,
        AttrData::U32(vec![10, 11, 12, 13, 10, 11, 12, 13])
    );
    assert_eq!(
        validate(&inset, &context()).unwrap().euler_characteristic,
        1
    );
}

#[test]
fn g6_30_inset_refuses_nonconvex_invalid_and_too_large_offsets() {
    let mesh = square();
    let too_large = inset_face(&mesh, 0, 1.1, &context());
    assert!(
        matches!(
            too_large,
            Err(GeometryError::PolyRefusal {
                code: "poly.inset_too_large",
                ..
            })
        ),
        "{too_large:?}"
    );
    assert!(matches!(
        inset_face(&mesh, 0, 0.0, &context()),
        Err(GeometryError::PolyRefusal { .. })
    ));
    assert!(matches!(
        inset_face(&mesh, 8, 0.1, &context()),
        Err(GeometryError::PolyRefusal {
            code: "poly.selection_invalid",
            ..
        })
    ));
    let concave = PolyMesh::new(
        vec![
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [1.0, 0.5, 0.0],
            [2.0, 2.0, 0.0],
            [0.0, 2.0, 0.0],
        ],
        vec![5],
        vec![0, 1, 2, 3, 4],
    )
    .unwrap();
    assert!(matches!(
        inset_face(&concave, 0, 0.1, &context()),
        Err(GeometryError::PolyRefusal {
            code: "poly.face_nonconvex",
            ..
        })
    ));
}

#[test]
fn g6_16_inset_scales_to_large_input_and_observes_cancellation() {
    let large = grid(317);
    assert_eq!(large.face_count(), 100_489);
    let result = inset_face(&large, 50_000, 0.2, &context()).unwrap();
    assert_eq!(result.face_count(), large.face_count() + 4);
    let cancelled = context();
    cancelled.cancel();
    assert!(matches!(
        inset_face(&square(), 0, 0.1, &cancelled),
        Err(GeometryError::Cancelled)
    ));
}
