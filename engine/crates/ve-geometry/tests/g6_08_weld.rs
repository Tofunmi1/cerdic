//! Acceptance tests for deterministic G6 vertex welding.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup and assertions should stop at their first failure"
)]

use ve_geometry::{
    AttrData, Attribute, Domain, Element, GeometryError, PolyMesh, Topology, validate, weld,
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

fn triangulated_grid(side: u32) -> PolyMesh {
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
    let mut face_sizes = Vec::new();
    let mut corners = Vec::new();
    for y in 0..side {
        for x in 0..side {
            let a = y * (side + 1) + x;
            let b = a + 1;
            let d = a + side + 1;
            let c = d + 1;
            face_sizes.extend([3, 3]);
            corners.extend([a, b, c, a, c, d]);
        }
    }
    PolyMesh::new(positions, face_sizes, corners).unwrap()
}

#[test]
fn g6_08_weld_uses_stable_representatives_and_keeps_corner_seams() {
    let mut split = mesh(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 0.0, 0.0],
        ],
        &[&[0, 1, 2], &[5, 4, 3]],
    );
    split
        .set_attribute(
            "point_id",
            Attribute {
                domain: Domain::Point,
                data: AttrData::U32(vec![10, 11, 12, 13, 14, 15]),
            },
        )
        .unwrap();
    split
        .set_attribute(
            "corner_id",
            Attribute {
                domain: Domain::Corner,
                data: AttrData::U32(vec![0, 1, 2, 3, 4, 5]),
            },
        )
        .unwrap();
    let result = weld(&split, 1.0e-5, 16, &context()).unwrap();
    assert_eq!(result.mesh.positions().len(), 4);
    assert_eq!(result.pairs_tested, 2);
    assert_eq!(
        result.mesh.attributes()["point_id"].data,
        AttrData::U32(vec![10, 11, 12, 13])
    );
    assert_eq!(
        result.mesh.attributes()["corner_id"].data,
        AttrData::U32(vec![0, 1, 2, 3, 4, 5])
    );
    assert!(
        validate(&result.mesh, &context())
            .unwrap()
            .nonmanifold_edges
            .is_empty()
    );
}

#[test]
fn g6_08_weld_refuses_face_collapse_and_candidate_pair_budget_overflow() {
    let collapsing = mesh(
        vec![[0.0, 0.0, 0.0], [0.01, 0.0, 0.0], [1.0, 0.0, 0.0]],
        &[&[0, 1, 2]],
    );
    assert!(matches!(
        weld(&collapsing, 0.02, 4, &context()),
        Err(GeometryError::PolyRefusal {
            code: "poly.weld_collapses_face",
            element: Some(Element::Face(0)),
        })
    ));
    let close_pair = mesh(
        vec![[0.0, 0.0, 0.0], [0.001, 0.0, 0.0], [1.0, 0.0, 0.0]],
        &[&[0, 1, 2]],
    );
    assert!(matches!(
        weld(&close_pair, 0.01, 0, &context()),
        Err(GeometryError::PolyRefusal {
            code: "poly.weld_pair_budget",
            element: None,
        })
    ));
}

#[test]
fn g6_08_weld_refuses_conflicting_edge_attributes() {
    let mut split = mesh(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 0.0, 0.0],
        ],
        &[&[0, 1, 2], &[5, 4, 3]],
    );
    split
        .set_attribute(
            "edge_tag",
            Attribute {
                domain: Domain::Edge,
                data: AttrData::U32((0..u32::try_from(split.edge_count()).unwrap()).collect()),
            },
        )
        .unwrap();
    assert!(matches!(
        weld(&split, 1.0e-5, 16, &context()),
        Err(GeometryError::PolyRefusal {
            code: "poly.attribute_conflict",
            element: Some(Element::Edge(_, _)),
        })
    ));
}

#[test]
fn g6_08_weld_preserves_unrelated_sliver_faces() {
    let sliver = mesh(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0e-8, 0.0],
            [0.0, 0.0, 1.0],
            [10.0, 10.0, 10.0],
            [10.0, 10.0, 10.0],
        ],
        &[&[0, 2, 1], &[0, 1, 3], &[1, 2, 3], &[2, 0, 3]],
    );
    let result = weld(&sliver, 1.0e-9, 4, &context()).unwrap();
    assert_eq!(result.mesh.positions().len(), 5);
    assert_eq!(
        validate(&result.mesh, &context()).unwrap().degenerate_faces,
        [0, 2]
    );
}

#[test]
fn g6_16_weld_large_grid_preflights_zero_candidate_pairs() {
    let grid = triangulated_grid(224);
    let result = weld(&grid, 0.01, 1_000, &context()).unwrap();
    assert_eq!(result.mesh.face_count(), 100_352);
    assert_eq!(result.pairs_tested, 0);
    assert_eq!(
        Topology::build(&result.mesh, &context())
            .unwrap()
            .edges()
            .len(),
        150_976
    );
}

#[test]
fn g6_16_weld_observes_cancellation() {
    let grid = triangulated_grid(8);
    let context = context();
    context.cancel();
    assert!(matches!(
        weld(&grid, 0.01, 10_000, &context),
        Err(GeometryError::Cancelled)
    ));
}
