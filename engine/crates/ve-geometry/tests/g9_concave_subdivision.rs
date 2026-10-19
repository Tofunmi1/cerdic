//! G9 regression checks for concave polygon subdivision.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup and assertions should stop at their first failure"
)]
use ve_geometry::{PolyMesh, SubdivisionOptions, subdivide, validate};
use ve_kernel::ExecutionContext;

fn options() -> SubdivisionOptions {
    SubdivisionOptions {
        levels: 1,
        boundary_rule: 0,
        crease_method: 0,
        creases: Vec::new(),
        sharp_corners: Vec::new(),
    }
}

fn concave_prism() -> PolyMesh {
    let profile = [
        [0.0, 0.0],
        [2.0, 0.0],
        [2.0, 2.0],
        [1.0, 0.8],
        [0.0, 2.0],
        [-0.5, 1.0],
        [-0.5, 0.25],
    ];
    let mut positions: Vec<[f32; 3]> = profile.iter().map(|p| [p[0], p[1], 0.0]).collect();
    positions.extend(profile.iter().map(|p| [p[0], p[1], 1.0]));
    let n = u32::try_from(profile.len()).unwrap();
    let mut corners: Vec<u32> = (0..n).rev().collect();
    corners.extend(n..2 * n);
    for index in 0..n {
        let next = (index + 1) % n;
        corners.extend([index, next, next + n, index + n]);
    }
    PolyMesh::new(
        positions,
        [vec![n; 2], vec![4; n as usize]].concat(),
        corners,
    )
    .unwrap()
}

fn convex_prism() -> PolyMesh {
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
    PolyMesh::new(
        positions,
        vec![4; 6],
        vec![
            0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 3, 7, 6, 2, 0, 4, 7, 3, 1, 2, 6, 5,
        ],
    )
    .unwrap()
}

#[test]
fn g9_a3_concave_prism_is_triangulated_and_remains_a_closed_solid() {
    let context = ExecutionContext::new().unwrap();
    let result = subdivide(&concave_prism(), options(), &context).unwrap();
    assert_eq!(result.triangulated_faces, 2);
    assert_eq!(result.mesh.face_count(), 58);
    assert_eq!(validate(&result.mesh, &context).unwrap().boundary_loops, 0);
    let (mesh, _) = result
        .mesh
        .to_kernel_mesh(ve_geometry::LossPolicy::Report)
        .unwrap();
    assert!(ve_kernel::Solid::from_mesh(&mesh).is_ok());
}

#[test]
fn g9_a3_convex_prism_keeps_its_original_quad_cage() {
    let context = ExecutionContext::new().unwrap();
    let result = subdivide(&convex_prism(), options(), &context).unwrap();
    assert_eq!(result.triangulated_faces, 0);
    assert_eq!(result.mesh.face_sizes(), &[4; 24]);
}
