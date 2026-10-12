//! Acceptance case G1a-18: kernel round trips keep what they can hold and refuse or report the rest.
#![expect(
    clippy::unwrap_used,
    reason = "the shared helper sits outside #[test] functions; failing loudly is the point"
)]

use ve_geometry::{
    AttrData, Attribute, Domain, GeometryError, LossPolicy, PolyMesh, polymesh_to_solid,
    solid_to_polymesh,
};
use ve_kernel::Solid;

fn cube_solid() -> Solid {
    Solid::cube([1.0, 2.0, 3.0], false).unwrap()
}

#[test]
fn g1a_18a_a_solid_survives_a_round_trip_through_a_polygon_mesh() {
    let solid = cube_solid();
    let mesh = solid_to_polymesh(&solid).unwrap();
    assert_eq!(mesh.face_count(), solid.triangle_count());
    assert_eq!(mesh.positions().len(), solid.vertex_count());
    let (back, report) = polymesh_to_solid(&mesh, LossPolicy::Reject).unwrap();
    assert!(report.dropped_attributes.is_empty());
    assert!((back.volume() - 6.0).abs() < 1e-6, "{}", back.volume());
    assert_eq!(back.triangle_count(), solid.triangle_count());
}

#[test]
fn g1a_18b_attributes_are_rejected_or_reported_never_dropped_silently() {
    let mut mesh = solid_to_polymesh(&cube_solid()).unwrap();
    let weight = Attribute {
        domain: Domain::Point,
        data: AttrData::F32(vec![0.5; mesh.positions().len()]),
    };
    mesh.set_attribute("weight", weight).unwrap();
    let slot = Attribute {
        domain: Domain::Face,
        data: AttrData::U32(vec![1; mesh.face_count()]),
    };
    mesh.set_attribute("material", slot).unwrap();

    match polymesh_to_solid(&mesh, LossPolicy::Reject) {
        Err(GeometryError::AttributesWouldBeLost(names)) => {
            assert_eq!(names, ["material", "weight"]);
        }
        other => panic!("{other:?}"),
    }
    let (solid, report) = polymesh_to_solid(&mesh, LossPolicy::Report).unwrap();
    assert_eq!(report.dropped_attributes, ["material", "weight"]);
    assert!((solid.volume() - 6.0).abs() < 1e-6);
}

#[test]
fn g1a_18c_quads_are_triangulated_and_the_split_is_reported() {
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
    let mesh = PolyMesh::new(positions, vec![4; 6], corners).unwrap();
    let (kernel, report) = mesh.to_kernel_mesh(LossPolicy::Reject).unwrap();
    assert_eq!(kernel.triangle_count(), 12);
    assert_eq!(report.faces_triangulated, 6);
    let (solid, _) = polymesh_to_solid(&mesh, LossPolicy::Reject).unwrap();
    assert!((solid.volume() - 1.0).abs() < 1e-6);
}

#[test]
fn g1a_18d_an_open_mesh_is_a_valid_polymesh_but_not_a_solid() {
    let positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let mesh = PolyMesh::new(positions, vec![3], vec![0, 1, 2]).unwrap();
    assert_eq!(mesh.edge_count(), 3);
    assert!(matches!(
        polymesh_to_solid(&mesh, LossPolicy::Reject),
        Err(GeometryError::Kernel(_))
    ));
}
