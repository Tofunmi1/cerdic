//! Geometric self-intersection checks for operators that create new surface.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup and assertions should stop at their first failure"
)]

use ve_geometry::{Element, GeometryError, PolyMesh, extrude_faces, inset_face};
use ve_kernel::ExecutionContext;

fn context() -> ExecutionContext {
    ExecutionContext::new().unwrap()
}

/// A small quad at z = 0 and a large one above it at z = 1.5, both facing up, as separate shells.
fn two_plates() -> PolyMesh {
    let positions = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [-2.0, -2.0, 1.5],
        [3.0, -2.0, 1.5],
        [3.0, 3.0, 1.5],
        [-2.0, 3.0, 1.5],
    ];
    PolyMesh::new(positions, vec![4, 4], vec![0, 1, 2, 3, 4, 5, 6, 7]).unwrap()
}

#[test]
fn g6_36_extruding_through_another_surface_is_refused_naming_the_face() {
    let result = extrude_faces(&two_plates(), &[0], [0.0, 0.0, 2.0], &context());
    assert!(
        matches!(
            result,
            Err(GeometryError::PolyRefusal {
                code: "poly.self_intersection",
                element: Some(Element::Face(_))
            })
        ),
        "{result:?}"
    );
}

#[test]
fn g6_36_extruding_short_of_the_other_surface_succeeds() {
    let result = extrude_faces(&two_plates(), &[0], [0.0, 0.0, 1.0], &context()).unwrap();
    assert_eq!(result.face_count(), 6);
}

#[test]
fn g6_36_extruding_into_a_coplanar_overlap_is_refused() {
    // The cap would land exactly on the large plate: two overlapping coplanar faces.
    let result = extrude_faces(&two_plates(), &[0], [0.0, 0.0, 1.5], &context());
    assert!(
        matches!(
            result,
            Err(GeometryError::PolyRefusal {
                code: "poly.self_intersection",
                ..
            })
        ),
        "{result:?}"
    );
}

#[test]
fn g6_36_an_inset_that_stays_inside_its_face_is_not_a_crossing() {
    let result = inset_face(&two_plates(), 1, 0.5, &context()).unwrap();
    assert_eq!(result.face_count(), 2 + 4);
}

#[test]
fn g6_36_the_check_observes_cancellation() {
    let cancelled = context();
    cancelled.cancel();
    assert!(matches!(
        extrude_faces(&two_plates(), &[0], [0.0, 0.0, 1.0], &cancelled),
        Err(GeometryError::Cancelled)
    ));
}
