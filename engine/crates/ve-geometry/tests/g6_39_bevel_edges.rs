//! Acceptance tests for edge beveling (v1: disjoint edges, valence-3 corners, 1 to 4 segments).
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "fixture setup and assertions should stop at their first failure"
)]

use ve_geometry::{
    AttrData, Attribute, Domain, Element, GeometryError, LossPolicy, PolyMesh, bevel_edges,
    polymesh_to_solid, validate,
};
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

fn volume(mesh: &PolyMesh) -> f64 {
    polymesh_to_solid(mesh, LossPolicy::Report)
        .unwrap()
        .0
        .volume()
}

fn code(result: &Result<PolyMesh, GeometryError>) -> &'static str {
    match result {
        Err(GeometryError::PolyRefusal { code, .. }) => code,
        other => panic!("expected a refusal, got {:?}", other.as_ref().map(|_| ())),
    }
}

#[test]
fn g6_39_a_chamfer_removes_exactly_the_corner_prism() {
    let width = 0.2;
    let result = bevel_edges(&cube(), &[(0, 1)], width, 1, &context()).unwrap();
    let report = validate(&result, &context()).unwrap();
    assert_eq!(report.euler_characteristic, 2);
    assert!(report.nonmanifold_edges.is_empty() && report.inconsistent_winding_edges.is_empty());
    assert_eq!(result.face_count(), 7);
    let expected = 1.0 - width * width / 2.0;
    assert!(
        (volume(&result) - expected).abs() < 1.0e-5,
        "{} vs {expected}",
        volume(&result)
    );
}

#[test]
fn g6_39_a_rounded_bevel_removes_between_the_circle_and_the_chamfer() {
    let width: f64 = 0.3;
    for segments in 2..=4 {
        let result = bevel_edges(&cube(), &[(0, 1)], width, segments, &context()).unwrap();
        let report = validate(&result, &context()).unwrap();
        assert_eq!(report.euler_characteristic, 2, "{segments} segments");
        assert_eq!(result.face_count(), 6 + segments);
        let removed = 1.0 - volume(&result);
        let circle = width * width * (1.0 - std::f64::consts::FRAC_PI_4);
        assert!(
            removed > circle - 1.0e-6 && removed < width * width / 2.0,
            "{segments} segments removed {removed}"
        );
    }
}

#[test]
fn g6_39_two_disjoint_edges_bevel_together() {
    let width = 0.2;
    let result = bevel_edges(&cube(), &[(0, 1), (6, 7)], width, 1, &context()).unwrap();
    assert_eq!(result.face_count(), 8);
    assert!((volume(&result) - (1.0 - width * width)).abs() < 1.0e-5);
}

#[test]
fn g6_39_four_disjoint_vertical_edges_round_a_block_that_shares_its_top_and_bottom_faces() {
    // Every vertical edge ends on the top and bottom faces, so those faces are rewritten four times.
    let width: f64 = 0.15;
    let result = bevel_edges(
        &cube(),
        &[(0, 4), (1, 5), (2, 6), (3, 7)],
        width,
        3,
        &context(),
    )
    .unwrap();
    let report = validate(&result, &context()).unwrap();
    assert_eq!(report.euler_characteristic, 2);
    assert!(report.nonmanifold_edges.is_empty() && report.inconsistent_winding_edges.is_empty());
    assert_eq!(result.face_count(), 6 + 4 * 3);
    let removed = 1.0 - volume(&result);
    let circle = 4.0 * width * width * (1.0 - std::f64::consts::FRAC_PI_4);
    assert!(
        removed > circle - 1.0e-6 && removed < 4.0 * width * width / 2.0,
        "removed {removed}"
    );
}

#[test]
fn g6_39_edges_sharing_a_vertex_are_a_network_and_refused() {
    let result = bevel_edges(&cube(), &[(0, 1), (1, 2)], 0.1, 1, &context());
    assert_eq!(code(&result), "poly.bevel_network_unsupported");
}

#[test]
fn g6_39_a_width_that_would_eat_the_adjacent_edges_is_refused() {
    let result = bevel_edges(&cube(), &[(0, 1)], 0.95, 1, &context());
    assert_eq!(code(&result), "poly.bevel_too_large");
}

#[test]
fn g6_39_bad_parameters_are_refused_before_any_work() {
    assert_eq!(
        code(&bevel_edges(&cube(), &[], 0.1, 1, &context())),
        "poly.selection_empty"
    );
    assert_eq!(
        code(&bevel_edges(&cube(), &[(0, 1)], 0.0, 1, &context())),
        "poly.bevel_width_invalid"
    );
    assert_eq!(
        code(&bevel_edges(&cube(), &[(0, 1)], f64::NAN, 1, &context())),
        "poly.bevel_width_invalid"
    );
    assert_eq!(
        code(&bevel_edges(&cube(), &[(0, 1)], 0.1, 0, &context())),
        "poly.bevel_segments_invalid"
    );
    assert_eq!(
        code(&bevel_edges(&cube(), &[(0, 1)], 0.1, 5, &context())),
        "poly.bevel_segments_invalid"
    );
    assert_eq!(
        code(&bevel_edges(&cube(), &[(0, 6)], 0.1, 1, &context())),
        "poly.selection_invalid"
    );
}

#[test]
fn g6_39_boundary_flat_and_high_valence_edges_are_refused_naming_the_element() {
    // Two coplanar quads: the shared edge is flat, the outer edges are boundary.
    let flat = PolyMesh::new(
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
    assert_eq!(
        code(&bevel_edges(&flat, &[(1, 4)], 0.1, 1, &context())),
        "poly.bevel_flat_edge"
    );
    let boundary = bevel_edges(&flat, &[(0, 1)], 0.1, 1, &context());
    assert!(matches!(
        boundary,
        Err(GeometryError::PolyRefusal {
            code: "poly.boundary_edge",
            element: Some(Element::Edge(0, 1))
        })
    ));
    // An octahedron's corners have four faces.
    let octahedron = PolyMesh::new(
        vec![
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
        ],
        vec![3; 8],
        vec![
            0, 2, 4, 2, 1, 4, 1, 3, 4, 3, 0, 4, 2, 0, 5, 1, 2, 5, 3, 1, 5, 0, 3, 5,
        ],
    )
    .unwrap();
    assert_eq!(
        code(&bevel_edges(&octahedron, &[(0, 2)], 0.05, 1, &context())),
        "poly.bevel_vertex_unsupported"
    );
}

#[test]
fn g6_39_attributes_follow_their_source_elements() {
    let mut source = cube();
    source
        .set_attribute(
            "point_id",
            Attribute {
                domain: Domain::Point,
                data: AttrData::U32((0..8).collect()),
            },
        )
        .unwrap();
    source
        .set_attribute(
            "face_id",
            Attribute {
                domain: Domain::Face,
                data: AttrData::U32((10..16).collect()),
            },
        )
        .unwrap();
    source
        .set_attribute(
            "corner_id",
            Attribute {
                domain: Domain::Corner,
                data: AttrData::U32((0..24).collect()),
            },
        )
        .unwrap();
    let result = bevel_edges(&source, &[(0, 1)], 0.2, 2, &context()).unwrap();
    let faces = &result.attributes()["face_id"].data;
    let AttrData::U32(faces) = faces else {
        panic!("face attribute type changed")
    };
    assert_eq!(faces.len(), result.face_count());
    assert_eq!(&faces[..6], &[10, 11, 12, 13, 14, 15]);
    assert!(
        faces[6..].iter().all(|&id| id == 10 || id == 12),
        "strip faces take the first edge face's value"
    );
    assert_eq!(
        result.attributes()["corner_id"].data.len(),
        result.corners().len()
    );
    assert_eq!(
        result.attributes()["point_id"].data.len(),
        result.positions().len()
    );
}

#[test]
fn g6_39_a_bevel_observes_cancellation_and_scales_to_a_large_mesh() {
    let cancelled = context();
    cancelled.cancel();
    assert!(matches!(
        bevel_edges(&cube(), &[(0, 1)], 0.1, 1, &cancelled),
        Err(GeometryError::Cancelled)
    ));
    // A cube beside a 100,000-quad sheet: the bevel is local, the checks are linear.
    let side = 317_u32;
    let mut positions = cube().positions().to_vec();
    let mut corners = cube().corners().to_vec();
    let mut sizes = vec![4_u32; 6];
    let base = u32::try_from(positions.len()).unwrap();
    for y in 0..=side {
        for x in 0..=side {
            positions.push([
                10.0 + f32::from(u16::try_from(x).unwrap()),
                f32::from(u16::try_from(y).unwrap()),
                0.0,
            ]);
        }
    }
    for y in 0..side {
        for x in 0..side {
            let a = base + y * (side + 1) + x;
            corners.extend([a, a + 1, a + side + 2, a + side + 1]);
            sizes.push(4);
        }
    }
    let large = PolyMesh::new(positions, sizes, corners).unwrap();
    assert!(large.face_count() > 100_000);
    let result = bevel_edges(&large, &[(0, 1)], 0.2, 1, &context()).unwrap();
    assert_eq!(result.face_count(), large.face_count() + 1);
}
