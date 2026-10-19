//! Geometry adapter acceptance checks for G7 subdivision.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup and assertions should stop at their first failure"
)]
use ve_geometry::{
    AttrData, Attribute, Domain, GeometryError, PolyMesh, SubdivisionOptions, subdivide,
};
use ve_kernel::ExecutionContext;

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
    PolyMesh::new(
        positions,
        vec![4; 6],
        vec![
            0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 3, 7, 6, 2, 0, 4, 7, 3, 1, 2, 6, 5,
        ],
    )
    .unwrap()
}

fn options(levels: u32) -> SubdivisionOptions {
    SubdivisionOptions {
        levels,
        boundary_rule: 0,
        crease_method: 0,
        creases: Vec::new(),
        sharp_corners: Vec::new(),
    }
}

#[test]
fn closed_cube_subdivision_returns_new_mesh_and_face_provenance() {
    let cage = cube();
    let mut cage = cage;
    cage.set_attribute(
        "region",
        Attribute {
            domain: Domain::Face,
            data: AttrData::U32(vec![0, 1, 2, 3, 4, 5]),
        },
    )
    .unwrap();
    let context = ExecutionContext::new().unwrap();
    let result = subdivide(&cage, options(1), &context).unwrap();
    assert_eq!(
        (result.mesh.positions().len(), result.mesh.face_count()),
        (26, 24)
    );
    assert_eq!(result.face_parent.len(), result.mesh.face_count());
    assert_eq!(
        result.vertex_origin_kind.len(),
        result.mesh.positions().len()
    );
    assert_eq!(cage.positions().len(), 8);
    assert_eq!(result.mesh.face_sizes(), &[4; 24]);
    assert_eq!(
        result.mesh.attributes()["region"].data,
        AttrData::U32((0..6).flat_map(|region| [region; 4]).collect())
    );
}

#[test]
fn rejects_nonmanifold_and_attribute_loss_explicitly() {
    let context = ExecutionContext::new().unwrap();
    let bad = PolyMesh::new(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
        ],
        vec![3, 3, 3],
        vec![0, 1, 2, 1, 0, 3, 0, 1, 4],
    )
    .unwrap();
    assert!(matches!(
        subdivide(&bad, options(1), &context),
        Err(GeometryError::PolyRefusal {
            code: "poly.subdivide.nonmanifold",
            ..
        })
    ));

    let mut attributed = cube();
    attributed
        .set_attribute(
            "weight",
            Attribute {
                domain: Domain::Point,
                data: AttrData::U32(vec![1; 8]),
            },
        )
        .unwrap();
    let result = subdivide(&attributed, options(1), &context).unwrap();
    assert_eq!(result.dropped_attributes, ["weight"]);
}

#[test]
fn unsupported_interpolation_domains_refuse_with_specific_codes() {
    let context = ExecutionContext::new().unwrap();
    let cases = [
        (
            "weight_f64",
            Domain::Point,
            AttrData::F64(vec![0.5; 8]),
            "poly.subdivide.f64_unsupported",
        ),
        (
            "edge_tag",
            Domain::Edge,
            AttrData::U32(vec![1; 12]),
            "poly.subdivide.edge_attribute_unsupported",
        ),
    ];
    for (name, domain, data, expected_code) in cases {
        let mut cage = cube();
        cage.set_attribute(name, Attribute { domain, data })
            .unwrap();
        assert!(matches!(
            subdivide(&cage, options(1), &context),
            Err(GeometryError::PolyRefusal { code, .. }) if code == expected_code
        ));
    }
    let mut uv_cage = cube();
    uv_cage
        .set_attribute(
            "u",
            Attribute {
                domain: Domain::Corner,
                data: AttrData::F32(vec![0.0; 24]),
            },
        )
        .unwrap();
    assert!(matches!(
        subdivide(&uv_cage, options(1), &context),
        Err(GeometryError::PolyRefusal {
            code: "poly.subdivide.uv_incomplete",
            ..
        })
    ));
    let mut arbitrary_corner = cube();
    arbitrary_corner
        .set_attribute(
            "crease_mask",
            Attribute {
                domain: Domain::Corner,
                data: AttrData::U32(vec![1; 24]),
            },
        )
        .unwrap();
    assert!(matches!(
        subdivide(&arbitrary_corner, options(1), &context),
        Err(GeometryError::PolyRefusal {
            code: "poly.subdivide.corner_attribute_unsupported",
            ..
        })
    ));
}

#[test]
fn hole_face_tags_refuse_until_the_subdivision_backend_propagates_them() {
    let context = ExecutionContext::new().unwrap();
    let mut cage = cube();
    cage.set_attribute(
        "hole",
        Attribute {
            domain: Domain::Face,
            data: AttrData::U32(vec![0, 0, 0, 0, 0, 1]),
        },
    )
    .unwrap();
    assert!(matches!(
        subdivide(&cage, options(1), &context),
        Err(GeometryError::PolyRefusal {
            code: "poly.subdivide.holes_unsupported",
            ..
        })
    ));
}

#[test]
fn inconsistent_winding_is_rejected_with_the_shared_edge() {
    let context = ExecutionContext::new().unwrap();
    let original = cube();
    let mut corners = original.corners().to_vec();
    corners[1..4].reverse();
    let miswound = PolyMesh::new(
        original.positions().to_vec(),
        original.face_sizes().to_vec(),
        corners,
    )
    .unwrap();
    assert!(matches!(
        subdivide(&miswound, options(1), &context),
        Err(GeometryError::PolyRefusal {
            code: "poly.subdivide.winding_conflict",
            element: Some(ve_geometry::Element::Edge(_, _)),
        })
    ));
}

#[test]
fn repeated_refinement_produces_four_children_per_face() {
    let context = ExecutionContext::new().unwrap();
    let output = subdivide(&cube(), options(2), &context).unwrap();
    assert_eq!(output.mesh.face_count(), 96);
    assert!(
        output
            .face_parent
            .iter()
            .enumerate()
            .all(|(index, parent)| *parent as usize == index / 16)
    );
}

#[test]
fn subdivision_quads_triangulate_to_a_closed_manifold() {
    let context = ExecutionContext::new().unwrap();
    let output = subdivide(&cube(), options(1), &context).unwrap();
    let (triangles, _) = output
        .mesh
        .to_kernel_mesh_with_context(ve_geometry::LossPolicy::Report, &context)
        .unwrap();
    assert_eq!(triangles.indices.len(), output.mesh.face_count() * 6);
    let topology = ve_geometry::validate(&output.mesh, &context).unwrap();
    assert_eq!(topology.boundary_loops, 0);
    assert!(ve_kernel::Solid::from_mesh(&triangles).is_ok());
}

#[test]
fn curved_quad_uses_the_shorter_diagonal_deterministically() {
    let context = ExecutionContext::new().unwrap();
    let mut quad = PolyMesh::new(
        vec![
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 2.0, 1.0],
            [0.0, 2.0, 0.0],
        ],
        vec![4],
        vec![0, 1, 2, 3],
    )
    .unwrap();
    quad.set_triangulation_mode(ve_geometry::TriangulationMode::CurvedQuads);
    let (triangles, _) = quad
        .to_kernel_mesh_with_context(ve_geometry::LossPolicy::Report, &context)
        .unwrap();
    assert!(
        triangles
            .indices
            .chunks_exact(3)
            .any(|face| { face.contains(&1) && face.contains(&3) })
    );
    let again = quad
        .to_kernel_mesh_with_context(ve_geometry::LossPolicy::Report, &context)
        .unwrap()
        .0;
    assert_eq!(triangles.indices, again.indices);
}
