//! B6: the `poly.bevel_edges` edge-network contract.
//!
//! V1 accepts a set of disjoint manifold interior edges whose end vertices have valence three. This
//! file pins that contract: connected networks, vertex fans, boundary and non-manifold edges, seams
//! and winding conflicts refuse atomically, leaving the input untouched, before the operator is ever
//! widened to edge networks.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "fixture setup and assertions should stop at their first failure"
)]

use ve_geometry::{Element, GeometryError, PolyMesh, bevel_edges};
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

fn code(result: Result<PolyMesh, GeometryError>) -> (&'static str, Option<Element>) {
    match result {
        Err(GeometryError::PolyRefusal { code, element }) => (code, element),
        other => panic!("expected a refusal, got {:?}", other.err()),
    }
}

#[test]
fn a_vertex_fan_of_edges_is_refused_as_a_network() {
    let cube = cube();
    // Three edges meeting at vertex 1: the operator reports the shared vertex and stops.
    let (code, element) = code(bevel_edges(
        &cube,
        &[(0, 1), (1, 2), (1, 5)],
        0.1,
        1,
        &context(),
    ));
    assert_eq!(code, "poly.bevel_network_unsupported");
    assert_eq!(element, Some(Element::Vertex(1)));
}

#[test]
fn a_non_manifold_edge_is_refused() {
    // Edge (0, 1) belongs to three triangles.
    let fin = PolyMesh::new(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
        ],
        vec![3, 3, 3],
        vec![0, 1, 2, 0, 1, 3, 0, 1, 4],
    )
    .unwrap();
    let (code, element) = code(bevel_edges(&fin, &[(0, 1)], 0.1, 1, &context()));
    assert_eq!(code, "poly.nonmanifold");
    assert_eq!(element, Some(Element::Edge(0, 1)));
}

#[test]
fn a_winding_conflict_is_refused() {
    // Two triangles share edge (0, 1) but both traverse it from 0 to 1.
    let conflicting = PolyMesh::new(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
        ],
        vec![3, 3],
        vec![0, 1, 2, 0, 1, 3],
    )
    .unwrap();
    let (code, element) = code(bevel_edges(&conflicting, &[(0, 1)], 0.1, 1, &context()));
    assert_eq!(code, "poly.winding_conflict");
    assert_eq!(element, Some(Element::Edge(0, 1)));
}

#[test]
fn a_seam_between_duplicated_vertices_reads_as_a_boundary() {
    // Two quads meeting along coincident but unwelded vertices: the seam edges are boundary edges, so
    // they refuse and the caller must weld first.
    let seam = PolyMesh::new(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
        ],
        vec![4, 4],
        vec![0, 1, 2, 3, 4, 5, 6, 7],
    )
    .unwrap();
    let (code, element) = code(bevel_edges(&seam, &[(1, 2)], 0.1, 1, &context()));
    assert_eq!(code, "poly.boundary_edge");
    assert_eq!(element, Some(Element::Edge(1, 2)));
}

#[test]
fn a_selection_mixing_valid_and_invalid_edges_refuses_atomically() {
    let cube = cube();
    let snapshot = cube.clone();
    // A valid edge then an edge that does not exist: the second refusal must not leave a partial
    // result, and the input keeps every vertex, corner and face.
    let (missing_code, missing_element) =
        code(bevel_edges(&cube, &[(0, 1), (0, 6)], 0.2, 1, &context()));
    assert_eq!(missing_code, "poly.selection_invalid");
    assert_eq!(missing_element, Some(Element::Edge(0, 6)));
    assert_eq!(cube, snapshot, "the input is unchanged after a refusal");

    // The same holds when the bad edge is a network partner of the first.
    let (network_code, network_element) =
        code(bevel_edges(&cube, &[(0, 1), (1, 2)], 0.2, 1, &context()));
    assert_eq!(network_code, "poly.bevel_network_unsupported");
    assert_eq!(network_element, Some(Element::Vertex(1)));
    assert_eq!(
        cube, snapshot,
        "the input is unchanged after a network refusal"
    );
}
