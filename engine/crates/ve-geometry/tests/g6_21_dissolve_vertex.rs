//! Acceptance tests for transactional vertex dissolution.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup and assertions should stop at their first failure"
)]

use ve_geometry::{Element, GeometryError, PolyMesh, dissolve_vertex};
use ve_kernel::ExecutionContext;

fn context() -> ExecutionContext {
    ExecutionContext::new().unwrap()
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
fn g6_21_dissolve_vertex_merges_an_interior_valence_four_fan() {
    let mesh = PolyMesh::new(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.5, 0.5, 0.0],
        ],
        vec![3; 4],
        vec![0, 1, 4, 1, 2, 4, 2, 3, 4, 3, 0, 4],
    )
    .unwrap();
    let result = dissolve_vertex(&mesh, 4, &context()).unwrap();
    assert_eq!(result.face_count(), 1);
    assert_eq!(result.positions().len(), 4);
    assert_eq!(result.face_sizes(), [4]);
}

#[test]
fn g6_23_dissolve_vertex_refuses_a_pinched_vertex() {
    let mesh = PolyMesh::new(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, -1.0, 0.0],
        ],
        vec![3; 2],
        vec![0, 1, 2, 0, 3, 4],
    )
    .unwrap();
    assert!(matches!(
        dissolve_vertex(&mesh, 0, &context()),
        Err(GeometryError::PolyRefusal {
            code: "poly.nonmanifold",
            element: Some(Element::Vertex(0)),
        })
    ));
}

#[test]
fn g6_22_dissolve_vertex_joins_two_boundary_edges() {
    let mesh = PolyMesh::new(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [2.0, 1.0, 0.0],
        ],
        vec![4, 4],
        vec![0, 1, 4, 3, 1, 2, 5, 4],
    )
    .unwrap();
    let result = dissolve_vertex(&mesh, 1, &context()).unwrap();
    assert_eq!(result.face_count(), 1);
    assert_eq!(result.face_sizes(), [5]);
    assert_eq!(result.positions().len(), 5);
}

#[test]
fn g6_16_dissolve_vertex_handles_large_input_and_cancellation() {
    let large = large_triangle_grid(224);
    let vertex = 112 * 225 + 112;
    let result = dissolve_vertex(&large, vertex, &context()).unwrap();
    assert_eq!(result.face_count(), large.face_count() - 5);

    let cancelled = context();
    cancelled.cancel();
    assert!(matches!(
        dissolve_vertex(&large, vertex, &cancelled),
        Err(GeometryError::Cancelled)
    ));
}
