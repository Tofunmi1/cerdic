//! G7 continuous point and UV subdivision attribute acceptance checks.
#![expect(
    clippy::unwrap_used,
    reason = "fixture setup and assertions should stop at their first failure"
)]
use ve_geometry::{
    AttrData, Attribute, Domain, GeometryError, PolyMesh, SubdivisionOptions, grid, subdivide,
};
use ve_kernel::{Crease, ExecutionContext};

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
fn point_float_channels_follow_geometry_and_keep_names() {
    let context = ExecutionContext::new().unwrap();
    let mut cage = cube();
    cage.set_attribute(
        "x_weight",
        Attribute {
            domain: Domain::Point,
            data: AttrData::F32(cage.positions().iter().map(|point| point[0]).collect()),
        },
    )
    .unwrap();
    cage.set_attribute(
        "constant_weight",
        Attribute {
            domain: Domain::Point,
            data: AttrData::F32(vec![2.5; cage.positions().len()]),
        },
    )
    .unwrap();
    let mut refined_options = options(1);
    refined_options.creases.push(Crease {
        vertices: [0, 1],
        sharpness: 2.0,
    });
    let output = subdivide(&cage, refined_options, &context).unwrap();
    let AttrData::F32(x_values) = &output.mesh.attributes()["x_weight"].data else {
        panic!("x_weight must remain F32");
    };
    let AttrData::F32(constant_values) = &output.mesh.attributes()["constant_weight"].data else {
        panic!("constant_weight must remain F32");
    };
    assert_eq!(
        x_values,
        &output
            .mesh
            .positions()
            .iter()
            .map(|point| point[0])
            .collect::<Vec<_>>()
    );
    assert!(
        constant_values
            .iter()
            .all(|value| value.to_bits() == 2.5_f32.to_bits())
    );
}

#[test]
fn point_channel_count_is_bounded() {
    let context = ExecutionContext::new().unwrap();
    let mut cage = cube();
    for channel in 0..9 {
        cage.set_attribute(
            &format!("weight_{channel}"),
            Attribute {
                domain: Domain::Point,
                data: AttrData::F32(vec![
                    if channel == 0 { 0.0 } else { 1.0 };
                    cage.positions().len()
                ]),
            },
        )
        .unwrap();
    }
    assert!(matches!(
        subdivide(&cage, options(1), &context),
        Err(GeometryError::PolyRefusal {
            code: "poly.subdivide.point_channel_limit",
            ..
        })
    ));
}

#[test]
fn uv_grid_keeps_identity_and_per_face_islands_stay_separate() {
    uv_square_keeps_identity(&context());
    cube_uv_islands_stay_separate(&context());
}

fn context() -> ExecutionContext {
    ExecutionContext::new().unwrap()
}

fn uv_square_keeps_identity(context: &ExecutionContext) {
    let mut square = PolyMesh::new(
        vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ],
        vec![4],
        vec![0, 1, 2, 3],
    )
    .unwrap();
    for (name, coordinate) in [("u", 0), ("v", 1)] {
        square
            .set_attribute(
                name,
                Attribute {
                    domain: Domain::Corner,
                    data: AttrData::F32(
                        square
                            .corners()
                            .iter()
                            .map(|&index| square.positions()[index as usize][coordinate])
                            .collect(),
                    ),
                },
            )
            .unwrap();
    }
    let mut square_options = options(2);
    square_options.boundary_rule = 2;
    let square_output = subdivide(&square, square_options, context).unwrap();
    let AttrData::F32(u) = &square_output.mesh.attributes()["u"].data else {
        unreachable!()
    };
    let AttrData::F32(v) = &square_output.mesh.attributes()["v"].data else {
        unreachable!()
    };
    for (corner, &point) in square_output.mesh.corners().iter().enumerate() {
        let position = square_output.mesh.positions()[point as usize];
        assert!((u[corner] - position[0]).abs() < 1.0e-6);
        assert!((v[corner] - position[1]).abs() < 1.0e-6);
    }
}

fn cube_uv_islands_stay_separate(context: &ExecutionContext) {
    let mut islands = cube();
    let local_uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let mut u_values = Vec::new();
    let mut v_values = Vec::new();
    for face in 0..6 {
        for uv in local_uv {
            u_values.push(uv[0] + [0.0, 2.0, 4.0, 6.0, 8.0, 10.0][face]);
            v_values.push(uv[1]);
        }
    }
    for (name, values) in [("u", u_values), ("v", v_values)] {
        islands
            .set_attribute(
                name,
                Attribute {
                    domain: Domain::Corner,
                    data: AttrData::F32(values),
                },
            )
            .unwrap();
    }
    let mut island_options = options(1);
    island_options.creases.push(Crease {
        vertices: [0, 1],
        sharpness: 1.0,
    });
    let island_output = subdivide(&islands, island_options, context).unwrap();
    let AttrData::F32(u_values) = &island_output.mesh.attributes()["u"].data else {
        unreachable!()
    };
    let mut shared_vertex_uv = Vec::new();
    let mut corner_start = 0;
    for (face, &parent) in island_output.face_parent.iter().enumerate() {
        let corner_end = corner_start + island_output.mesh.face_sizes()[face] as usize;
        for (corner, &u_value) in u_values
            .iter()
            .enumerate()
            .take(corner_end)
            .skip(corner_start)
        {
            let vertex = island_output.mesh.corners()[corner] as usize;
            if island_output.vertex_origin_kind[vertex] == 0
                && island_output.vertex_origin_index[vertex] == 0
            {
                shared_vertex_uv.push((parent, u_value.to_bits()));
            }
        }
        corner_start = corner_end;
    }
    assert!(shared_vertex_uv.len() >= 3, "{shared_vertex_uv:?}");
    assert!(
        shared_vertex_uv
            .iter()
            .map(|entry| entry.1)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            >= 2,
        "{shared_vertex_uv:?}; all output UV u values: {u_values:?}"
    );
}

#[test]
fn generated_uv_grid_keeps_normalized_uvs_through_subdivision() {
    let context = ExecutionContext::new().unwrap();
    let cage = grid(2.0, 1.0, 2, 2).unwrap();
    let mut boundary_options = options(2);
    boundary_options.boundary_rule = 2;
    let result = subdivide(&cage, boundary_options, &context).unwrap();
    let AttrData::F32(u) = &result.mesh.attributes()["u"].data else {
        unreachable!()
    };
    let AttrData::F32(v) = &result.mesh.attributes()["v"].data else {
        unreachable!()
    };
    for (corner, &point) in result.mesh.corners().iter().enumerate() {
        let position = result.mesh.positions()[point as usize];
        assert!((u[corner] - (position[0] + 1.0) * 0.5).abs() < 1.0e-6);
        assert!((v[corner] - position[1]).abs() < 1.0e-6);
    }
}
