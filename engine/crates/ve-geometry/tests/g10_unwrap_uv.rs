//! `unwrap_uv`: Blender's parametrizer writes corner UVs as the attributes `u` and `v`.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "fixtures are known to be valid; failures are reported by panicking"
)]

use ve_geometry::{
    AttrData, Attribute, Domain, GeometryError, PolyMesh, UvMethod, UvUnwrap, unwrap_uv,
};
use ve_kernel::ExecutionContext;

fn cube() -> PolyMesh {
    PolyMesh::new(
        vec![
            [-1.0, -1.0, -1.0],
            [1.0, -1.0, -1.0],
            [1.0, 1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, -1.0, 1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, 1.0],
            [-1.0, 1.0, 1.0],
        ],
        vec![4; 6],
        vec![
            0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6, 3, 0, 4, 7,
        ],
    )
    .unwrap()
}

const ALL_EDGES: [(u32, u32); 12] = [
    (0, 3),
    (3, 2),
    (2, 1),
    (1, 0),
    (4, 5),
    (5, 6),
    (6, 7),
    (7, 4),
    (0, 4),
    (1, 5),
    (2, 6),
    (3, 7),
];

fn options(pack: bool) -> UvUnwrap {
    UvUnwrap {
        method: UvMethod::Conformal,
        pack,
        rotate: false,
        margin: 0.02,
    }
}

fn corner_values(mesh: &PolyMesh, name: &str) -> Vec<f32> {
    match &mesh.attributes()[name].data {
        AttrData::F32(values) => values.clone(),
        other => panic!("{name} is not f32: {other:?}"),
    }
}

#[test]
fn a_cube_cut_along_every_edge_gets_packed_uvs_in_the_unit_square() {
    let context = ExecutionContext::new().unwrap();
    let result = unwrap_uv(&cube(), &ALL_EDGES, &options(true), &context).unwrap();
    for name in ["u", "v"] {
        assert_eq!(result.attributes()[name].domain, Domain::Corner);
        let values = corner_values(&result, name);
        assert_eq!(values.len(), 24);
        assert!(
            values.iter().all(|v| (-1e-6..=1.0 + 1e-6).contains(v)),
            "{name}"
        );
    }
    // The six islands are laid out side by side (a grid of at least two columns), not stacked on one
    // another: each column of squares contributes two distinct u values.
    let mut u = corner_values(&result, "u");
    u.sort_by(f32::total_cmp);
    u.dedup_by(|a, b| (*a - *b).abs() < 1e-5);
    assert!(u.len() >= 4, "{} distinct u values", u.len());
}

#[test]
fn existing_attributes_are_kept_and_uvs_are_replaced() {
    let context = ExecutionContext::new().unwrap();
    let mut mesh = cube();
    mesh.set_attribute(
        "region",
        Attribute {
            domain: Domain::Face,
            data: AttrData::U32((0..6).collect()),
        },
    )
    .unwrap();
    mesh.set_attribute(
        "u",
        Attribute {
            domain: Domain::Corner,
            data: AttrData::F32(vec![9.0; 24]),
        },
    )
    .unwrap();
    let result = unwrap_uv(&mesh, &ALL_EDGES, &options(true), &context).unwrap();
    assert_eq!(result.attributes()["region"], mesh.attributes()["region"]);
    assert!(corner_values(&result, "u").iter().all(|&u| u <= 1.0));
}

#[test]
fn a_seam_that_is_not_an_edge_refuses_and_the_input_is_unchanged() {
    let context = ExecutionContext::new().unwrap();
    let mesh = cube();
    let before = mesh.clone();
    let error = unwrap_uv(&mesh, &[(0, 6)], &options(true), &context).unwrap_err();
    assert!(matches!(
        error,
        GeometryError::PolyRefusal {
            code: "poly.selection_invalid",
            ..
        }
    ));
    assert_eq!(mesh, before);
}

#[test]
fn a_cancelled_context_stops_before_the_unwrap() {
    let context = ExecutionContext::new().unwrap();
    context.cancel();
    let error = unwrap_uv(&cube(), &ALL_EDGES, &options(true), &context).unwrap_err();
    assert_eq!(error, GeometryError::Cancelled);
}
