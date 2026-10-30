//! Merging a solid's coplanar triangles back into its flat faces.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup and assertions should stop at their first failure"
)]

use ve_geometry::{GeometryError, PolyMesh, merge_coplanar, solid_to_polymesh, validate};
use ve_kernel::{ExecutionContext, Solid};

fn context() -> ExecutionContext {
    ExecutionContext::new().unwrap()
}

#[test]
fn g6_42_a_triangulated_box_becomes_six_quads_with_the_same_shape() {
    let triangles = solid_to_polymesh(&Solid::cube([1.0, 2.0, 3.0], true).unwrap()).unwrap();
    assert_eq!(triangles.face_count(), 12);
    let merged = merge_coplanar(&triangles, 0.01, &context()).unwrap();
    assert_eq!(merged.face_count(), 6);
    assert!(merged.face_sizes().iter().all(|&size| size == 4));
    let report = validate(&merged, &context()).unwrap();
    assert_eq!(report.euler_characteristic, 2);
    assert!(report.nonmanifold_edges.is_empty() && report.inconsistent_winding_edges.is_empty());
}

#[test]
fn g6_42_a_curved_mesh_has_no_coplanar_patches_and_is_unchanged() {
    let sphere = solid_to_polymesh(&Solid::sphere(1.0, 16).unwrap()).unwrap();
    let merged = merge_coplanar(&sphere, 0.01, &context()).unwrap();
    assert_eq!(merged.face_count(), sphere.face_count());
}

#[test]
fn g6_42_keeping_holes_merges_what_it_can_and_leaves_a_ring_of_faces_alone() {
    // A wall with a window: the two long faces have a hole (one patch that is not one loop), the four sides of the box and of the opening
    // are plain quads. Strict merging refuses it; merging that keeps holes merges every patch that can merge and leaves the others.
    let wall = Solid::cube([6.0, 2.8, 0.5], true)
        .unwrap()
        .difference(&Solid::cube([1.2, 1.2, 1.0], true).unwrap())
        .unwrap();
    let triangles = solid_to_polymesh(&wall).unwrap();
    assert!(
        merge_coplanar(&triangles, 0.01, &context()).is_err(),
        "strict merging refuses the faces with a hole"
    );
    let kept = ve_geometry::merge_coplanar_keeping_holes(&triangles, 0.01, &context()).unwrap();
    assert!(
        kept.face_count() < triangles.face_count(),
        "the plain patches merged: {} of {}",
        kept.face_count(),
        triangles.face_count()
    );
    let report = validate(&kept, &context()).unwrap();
    assert_eq!(
        report.euler_characteristic, 0,
        "still a solid with a hole through it"
    );
    assert!(report.nonmanifold_edges.is_empty() && report.inconsistent_winding_edges.is_empty());
}

#[test]
fn g6_42_keeping_holes_gives_the_same_as_merging_when_there_are_none() {
    let triangles = solid_to_polymesh(&Solid::cube([1.0, 2.0, 3.0], true).unwrap()).unwrap();
    let strict = merge_coplanar(&triangles, 0.01, &context()).unwrap();
    let kept = ve_geometry::merge_coplanar_keeping_holes(&triangles, 0.01, &context()).unwrap();
    assert_eq!(kept.face_count(), strict.face_count());
}

#[test]
fn g6_42_a_flat_patch_with_a_hole_is_refused_not_repaired() {
    // A ring of eight coplanar quads around an empty middle: one patch whose union is not one loop.
    let mut positions = Vec::new();
    for y in 0..4_u16 {
        for x in 0..4_u16 {
            positions.push([f32::from(x), f32::from(y), 0.0]);
        }
    }
    let mut corners = Vec::new();
    let mut sizes = Vec::new();
    for y in 0..3_u32 {
        for x in 0..3_u32 {
            if x == 1 && y == 1 {
                continue;
            }
            let a = y * 4 + x;
            corners.extend([a, a + 1, a + 5, a + 4]);
            sizes.push(4);
        }
    }
    let ring = PolyMesh::new(positions, sizes, corners).unwrap();
    let result = merge_coplanar(&ring, 0.01, &context());
    assert!(
        matches!(result, Err(GeometryError::PolyRefusal { .. })),
        "{:?}",
        result.map(|m| m.face_count())
    );
}

#[test]
fn g6_42_bad_angles_and_cancellation_are_refused() {
    let triangles = solid_to_polymesh(&Solid::cube([1.0; 3], true).unwrap()).unwrap();
    for angle in [-0.1, 0.6, f64::NAN] {
        assert!(matches!(
            merge_coplanar(&triangles, angle, &context()),
            Err(GeometryError::PolyRefusal {
                code: "poly.angle_invalid",
                ..
            })
        ));
    }
    let cancelled = context();
    cancelled.cancel();
    assert!(matches!(
        merge_coplanar(&triangles, 0.01, &cancelled),
        Err(GeometryError::Cancelled)
    ));
}
