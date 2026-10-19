//! Acceptance tests for G6 polygon topology contracts.
#![expect(
    clippy::unwrap_used,
    reason = "test fixtures and assertions should fail immediately when setup is invalid"
)]

use ve_geometry::{
    Element, GeometryError, LossPolicy, PolyMesh, Topology, flip_component, flip_faces, validate,
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
    mesh(
        positions,
        &[
            &[0, 3, 2, 1],
            &[4, 5, 6, 7],
            &[0, 1, 5, 4],
            &[3, 7, 6, 2],
            &[0, 4, 7, 3],
            &[1, 2, 6, 5],
        ],
    )
}

#[test]
fn g6_13_closed_cube_has_manifold_edges_and_euler_two() {
    let topology = Topology::build(&cube(), &context()).unwrap();
    assert_eq!(topology.edges().len(), 12);
    assert_eq!(topology.components().len(), 1);
    assert!(topology.isolated_vertices().is_empty());
    assert_eq!(topology.euler_characteristic(), 2);
    assert!(topology.edges().values().all(|edge| edge.face_count() == 2));
    assert!(validate(&cube(), &context()).is_ok());
}

#[test]
fn g6_16_topology_walk_observes_cancellation() {
    let context = context();
    context.cancel();
    assert!(matches!(
        Topology::build(&cube(), &context),
        Err(GeometryError::Cancelled)
    ));
}

#[test]
fn g6_10_whole_component_flip_reverses_winding_and_corner_values() {
    let mut original = cube();
    original
        .set_attribute(
            "corner_id",
            ve_geometry::Attribute {
                domain: ve_geometry::Domain::Corner,
                data: ve_geometry::AttrData::U32((0..24).collect()),
            },
        )
        .unwrap();
    let flipped = flip_component(&original, &[0, 1, 2, 3, 4, 5], &context()).unwrap();
    assert!(validate(&flipped, &context()).is_ok());
    assert_eq!(flipped.face_sizes(), original.face_sizes());
    assert_eq!(flipped.attributes()["corner_id"].data.len(), 24);
    assert_ne!(flipped.corners(), original.corners());
    assert_eq!(
        original.corners()[0],
        0,
        "the operation must leave its input unchanged"
    );
}

#[test]
fn g6_10_partial_component_flip_refuses_without_mutating_input() {
    let original = cube();
    let before = original.clone();
    assert!(matches!(
        flip_component(&original, &[0], &context()),
        Err(GeometryError::PolyRefusal {
            code: "poly.selection_splits_component",
            element: Some(Element::Face(1)),
        })
    ));
    assert_eq!(original, before);
}

#[test]
fn g6_19_partial_face_flip_repairs_existing_winding_conflict() {
    let miswound = mesh(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ],
        &[&[0, 1, 2], &[0, 3, 2]],
    );
    assert_eq!(
        validate(&miswound, &context())
            .unwrap()
            .inconsistent_winding_edges,
        [(0, 2)]
    );
    let result = flip_faces(&miswound, &[1], false, &context()).unwrap();
    assert!(result.allowed_inconsistencies.is_empty());
    assert!(
        validate(&result.mesh, &context())
            .unwrap()
            .inconsistent_winding_edges
            .is_empty()
    );
}

#[test]
fn g6_20_partial_face_flip_cannot_add_winding_conflicts_unless_explicitly_allowed() {
    let consistent = mesh(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ],
        &[&[0, 1, 2], &[0, 2, 3]],
    );
    let before = consistent.clone();
    assert!(matches!(
        flip_faces(&consistent, &[1], false, &context()),
        Err(GeometryError::PolyRefusal {
            code: "poly.flip_increases_inconsistency",
            element: Some(Element::Edge(0, 2)),
        })
    ));
    assert_eq!(consistent, before);
    let result = flip_faces(&consistent, &[1], true, &context()).unwrap();
    assert_eq!(result.allowed_inconsistencies, [(0, 2)]);
}

#[test]
fn g6_13_open_grid_has_one_closed_boundary_loop() {
    let positions = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
    ];
    let grid = mesh(positions, &[&[0, 1, 2], &[0, 2, 3]]);
    let topology = Topology::build(&grid, &context()).unwrap();
    assert_eq!(topology.boundary_loops().len(), 1);
    assert_eq!(topology.boundary_loops()[0].len(), 4);
    assert_eq!(topology.euler_characteristic(), 1);
    assert!(validate(&grid, &context()).is_ok());
}

#[test]
fn g6_13_components_and_isolated_vertices_are_reported_deterministically() {
    let positions = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [3.0, 0.0, 0.0],
        [4.0, 0.0, 0.0],
        [3.0, 1.0, 0.0],
        [8.0, 8.0, 8.0],
    ];
    let disconnected = mesh(positions, &[&[0, 1, 2], &[3, 4, 5]]);
    let topology = Topology::build(&disconnected, &context()).unwrap();
    assert_eq!(topology.components(), &[vec![0, 1, 2], vec![3, 4, 5]]);
    assert_eq!(topology.isolated_vertices(), &[6]);
}

#[test]
fn g6_13_vertex_touch_counts_as_one_vertex_connected_component() {
    let touching = mesh(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, -1.0, 0.0],
        ],
        &[&[0, 1, 2], &[0, 3, 4]],
    );
    let topology = Topology::build(&touching, &context()).unwrap();
    assert_eq!(topology.components(), &[vec![0, 1, 2, 3, 4]]);
}

#[test]
fn g6_13_repeated_corner_is_refused_and_f32_sliver_is_reported() {
    let repeated = mesh(
        vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        &[&[0, 1, 0, 2]],
    );
    assert!(validate(&repeated, &context()).is_err());

    let thin_closed = mesh(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0e-8, 0.0],
            [0.0, 0.0, 1.0],
        ],
        &[&[0, 2, 1], &[0, 1, 3], &[1, 2, 3], &[2, 0, 3]],
    );
    let report = validate(&thin_closed, &context()).unwrap();
    assert_eq!(report.degenerate_faces, [0, 2]);
    assert!(report.nonmanifold_edges.is_empty());
    assert!(report.inconsistent_winding_edges.is_empty());
}

#[test]
fn g6_02_concave_planar_polygon_ear_clips_without_filling_its_notch() {
    let concave = mesh(
        vec![
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 2.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 2.0, 0.0],
        ],
        &[&[0, 1, 2, 3, 4]],
    );
    let (triangles, report) = concave.to_kernel_mesh(LossPolicy::Report).unwrap();
    assert_eq!(triangles.indices.len(), 9);
    assert_eq!(report.faces_triangulated, 1);
    let area = triangles
        .indices
        .chunks_exact(3)
        .map(|face| {
            let a = triangles.positions[usize::try_from(face[0]).unwrap()];
            let b = triangles.positions[usize::try_from(face[1]).unwrap()];
            let c = triangles.positions[usize::try_from(face[2]).unwrap()];
            ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() * 0.5
        })
        .sum::<f32>();
    assert!((area - 3.0).abs() < 1.0e-6);
}

#[test]
fn g6_02_nonplanar_polygon_is_refused_instead_of_fan_triangulated() {
    let nonplanar = mesh(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.2],
            [0.0, 1.0, 0.0],
        ],
        &[&[0, 1, 2, 3]],
    );
    assert!(matches!(
        nonplanar.to_kernel_mesh(LossPolicy::Report),
        Err(GeometryError::TriangulationUnsupported(
            "non-planar polygon"
        ))
    ));
}

#[test]
fn g6_02_small_nonplanarity_is_preserved_as_triangles() {
    let slightly_warped = mesh(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 1.0e-4],
            [0.0, 1.0, 0.0],
        ],
        &[&[0, 1, 2, 3]],
    );
    let (triangles, _) = slightly_warped.to_kernel_mesh(LossPolicy::Report).unwrap();
    assert_eq!(triangles.indices.len(), 6);
}

#[test]
fn g6_16_triangulation_observes_cancellation() {
    let context = context();
    context.cancel();
    assert!(matches!(
        cube().to_kernel_mesh_with_context(LossPolicy::Report, &context),
        Err(GeometryError::Cancelled)
    ));
}

#[test]
fn g6_13_reversed_shared_edge_and_three_face_edge_are_distinguished() {
    let positions = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.5, -1.0, 0.0],
    ];
    let reversed = mesh(positions.clone(), &[&[0, 1, 2], &[0, 1, 3]]);
    let report = validate(&reversed, &context()).unwrap();
    assert_eq!(report.inconsistent_winding_edges, [(0, 1)]);

    let nonmanifold = mesh(positions, &[&[0, 1, 2], &[1, 0, 3], &[0, 1, 4]]);
    let topology = Topology::build(&nonmanifold, &context()).unwrap();
    assert_eq!(topology.edges()[&(0, 1)].face_count(), 3);
    let report = validate(&nonmanifold, &context()).unwrap();
    assert_eq!(report.nonmanifold_edges, [(0, 1, 3)]);
}

#[test]
fn g6_16_large_grid_builds_topology_without_quadratic_pair_scans() {
    let side = 224_u32;
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
    let mut faces = Vec::new();
    for y in 0..side {
        for x in 0..side {
            let a = y * (side + 1) + x;
            let b = a + 1;
            let d = a + side + 1;
            let c = d + 1;
            faces.push([a, b, c]);
            faces.push([a, c, d]);
        }
    }
    let mesh = mesh(
        positions,
        &faces.iter().map(<[u32; 3]>::as_slice).collect::<Vec<_>>(),
    );
    let topology = Topology::build(&mesh, &context()).unwrap();
    assert_eq!(mesh.face_count(), 100_352);
    assert_eq!(topology.edges().len(), 150_976);
    assert_eq!(topology.boundary_loops().len(), 1);
    let selected: Vec<_> = (0..mesh.face_count()).collect();
    let flipped = flip_faces(&mesh, &selected, false, &context()).unwrap();
    assert_eq!(flipped.mesh.face_count(), 100_352);

    let cancelled = context();
    cancelled.cancel();
    assert!(matches!(
        flip_faces(&mesh, &selected, false, &cancelled),
        Err(GeometryError::Cancelled)
    ));
}
