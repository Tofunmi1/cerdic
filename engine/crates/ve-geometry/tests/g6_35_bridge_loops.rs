//! Acceptance tests for deterministic equal-count boundary-loop bridging.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup and assertions should stop at their first failure"
)]

use ve_geometry::{Element, GeometryError, PolyMesh, Topology, bridge_loops, validate};
use ve_kernel::ExecutionContext;

fn context() -> ExecutionContext {
    ExecutionContext::new().unwrap()
}

fn two_quads() -> PolyMesh {
    PolyMesh::new(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ],
        vec![4, 4],
        vec![0, 1, 2, 3, 4, 5, 6, 7],
    )
    .unwrap()
}

#[test]
fn g6_35_bridge_equal_boundary_loops_creates_closed_component() {
    let mesh = two_quads();
    let result = bridge_loops(&mesh, 0, 1, &context()).unwrap();
    let report = validate(&result, &context()).unwrap();
    assert_eq!(result.face_count(), 6);
    assert_eq!(report.components, 1);
    assert_eq!(report.boundary_loops, 0);
    assert_eq!(report.euler_characteristic, 2);
    assert!(report.nonmanifold_edges.is_empty());
    assert!(report.inconsistent_winding_edges.is_empty());
}

#[test]
fn g6_36_bridge_refuses_unavailable_or_mismatched_boundary_loops() {
    assert!(matches!(
        bridge_loops(&two_quads(), 0, 3, &context()),
        Err(GeometryError::PolyRefusal {
            code: "poly.loop_not_boundary",
            ..
        })
    ));
    let mesh = PolyMesh::new(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [2.0, 0.0, 0.0],
            [3.0, 0.0, 0.0],
            [3.0, 1.0, 0.0],
            [2.0, 1.0, 0.0],
        ],
        vec![3, 4],
        vec![0, 1, 2, 3, 4, 5, 6],
    )
    .unwrap();
    assert!(matches!(
        bridge_loops(&mesh, 0, 1, &context()),
        Err(GeometryError::PolyRefusal {
            code: "poly.loop_mismatch",
            element: Some(Element::Vertex(_))
        })
    ));
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
    let mut sizes = vec![4; (side * side) as usize];
    let mut positions = points;
    let offset = u32::try_from(positions.len()).unwrap();
    positions.extend([
        [0.0, 0.0, 2.0],
        [1.0, 0.0, 2.0],
        [1.0, 1.0, 2.0],
        [0.0, 1.0, 2.0],
        [0.0, 0.0, 3.0],
        [1.0, 0.0, 3.0],
        [1.0, 1.0, 3.0],
        [0.0, 1.0, 3.0],
    ]);
    sizes.extend([4, 4]);
    corners.extend([
        offset,
        offset + 1,
        offset + 2,
        offset + 3,
        offset + 4,
        offset + 5,
        offset + 6,
        offset + 7,
    ]);
    PolyMesh::new(positions, sizes, corners).unwrap()
}

#[test]
fn g6_16_bridge_scales_and_observes_cancellation() {
    let mesh = grid(317);
    assert_eq!(mesh.face_count(), 100_491);
    let loops = Topology::build(&mesh, &context())
        .unwrap()
        .boundary_loops()
        .to_vec();
    let quad_loops: Vec<_> = loops
        .iter()
        .enumerate()
        .filter_map(|(i, loop_)| (loop_.len() == 4).then_some(i))
        .collect();
    assert_eq!(quad_loops.len(), 2);
    let result = bridge_loops(&mesh, quad_loops[0], quad_loops[1], &context()).unwrap();
    assert_eq!(result.face_count(), mesh.face_count() + 4);
    let cancelled = context();
    cancelled.cancel();
    assert!(matches!(
        bridge_loops(&two_quads(), 0, 1, &cancelled),
        Err(GeometryError::Cancelled)
    ));
}
