//! Acceptance tests for transactional face patch dissolution.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup and assertions should stop at their first failure"
)]

use ve_geometry::{AttrData, Attribute, Domain, Element, GeometryError, PolyMesh, dissolve_faces};
use ve_kernel::ExecutionContext;

fn context() -> ExecutionContext {
    ExecutionContext::new().unwrap()
}

fn quad_grid(side: u32, missing: Option<(u32, u32)>) -> PolyMesh {
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
            if missing == Some((x, y)) {
                continue;
            }
            let a = y * (side + 1) + x;
            let b = a + 1;
            let d = a + side + 1;
            let c = d + 1;
            sizes.push(4);
            corners.extend([a, b, c, d]);
        }
    }
    PolyMesh::new(positions, sizes, corners).unwrap()
}

fn large_triangle_grid(side: u32) -> PolyMesh {
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
    PolyMesh::new(positions, sizes, corners).unwrap()
}

#[test]
fn g6_24_dissolve_faces_merges_a_connected_quad_patch() {
    let mut source = quad_grid(2, None);
    source
        .set_attribute(
            "face_tag",
            Attribute {
                domain: Domain::Face,
                data: AttrData::U32(vec![9; 4]),
            },
        )
        .unwrap();
    source
        .set_attribute(
            "corner_uv_id",
            Attribute {
                domain: Domain::Corner,
                data: AttrData::U32((0..16).collect()),
            },
        )
        .unwrap();
    let result = dissolve_faces(&source, &[0, 1, 2, 3], &context()).unwrap();
    assert_eq!(result.face_count(), 1);
    assert_eq!(result.face_sizes(), [8]);
    assert_eq!(result.corners(), [0, 1, 2, 4, 7, 6, 5, 3]);
    assert_eq!(result.attributes()["face_tag"].data, AttrData::U32(vec![9]));
    assert_eq!(
        result.attributes()["corner_uv_id"].data,
        AttrData::U32(vec![0, 4, 5, 13, 14, 10, 11, 3])
    );
}

#[test]
fn g6_25_dissolve_faces_refuses_disconnected_and_holed_patches() {
    let disconnected = quad_grid(3, None);
    assert!(matches!(
        dissolve_faces(&disconnected, &[0, 8], &context()),
        Err(GeometryError::PolyRefusal {
            code: "poly.selection_disconnected",
            element: Some(Element::Face(_)),
        })
    ));
    let ring = quad_grid(3, Some((1, 1)));
    let selection: Vec<_> = (0..8).collect();
    let before = ring.clone();
    assert!(matches!(
        dissolve_faces(&ring, &selection, &context()),
        Err(GeometryError::PolyRefusal {
            code: "poly.loop_invalid",
            element: Some(Element::Face(_)),
        })
    ));
    assert_eq!(ring, before);
}

#[test]
fn g6_24_dissolve_faces_refuses_conflicting_face_attributes() {
    let mut source = quad_grid(2, None);
    source
        .set_attribute(
            "face_tag",
            Attribute {
                domain: Domain::Face,
                data: AttrData::U32(vec![1, 2, 1, 1]),
            },
        )
        .unwrap();
    let before = source.clone();
    assert!(matches!(
        dissolve_faces(&source, &[0, 1, 2, 3], &context()),
        Err(GeometryError::PolyRefusal {
            code: "poly.attribute_conflict",
            element: Some(Element::Face(1)),
        })
    ));
    assert_eq!(source, before);
}

#[test]
fn g6_16_dissolve_faces_handles_large_patch_and_cancellation() {
    let large = large_triangle_grid(224);
    assert_eq!(large.face_count(), 100_352);
    let selected: Vec<_> = (0..large.face_count()).collect();
    let result = dissolve_faces(&large, &selected, &context()).unwrap();
    assert_eq!(result.face_count(), 1);
    assert_eq!(result.face_sizes(), [896]);

    let cancelled = context();
    cancelled.cancel();
    assert!(matches!(
        dissolve_faces(&quad_grid(2, None), &[0, 1, 2, 3], &cancelled),
        Err(GeometryError::Cancelled)
    ));
}
