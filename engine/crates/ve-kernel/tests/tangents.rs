//! Tangent-space generation (`MikkTSpace`, vendored from Blender) behind `ve_kernel::tangents`.
use ve_kernel::{KernelError, Solid, tangents};

const QUAD_POSITIONS: [[f32; 3]; 4] = [
    [0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [1.0, 1.0, 0.0],
    [0.0, 1.0, 0.0],
];
const QUAD_NORMALS: [[f32; 3]; 4] = [[0.0, 0.0, 1.0]; 4];
const QUAD_INDICES: [u32; 6] = [0, 1, 2, 0, 2, 3];

fn close(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1.0e-5)
}

#[test]
fn a_quad_whose_u_follows_x_has_tangent_plus_x_and_positive_sign() {
    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let result = tangents(&QUAD_POSITIONS, &QUAD_NORMALS, &uvs, &QUAD_INDICES).expect("tangents");
    assert_eq!(result.len(), QUAD_INDICES.len());
    for corner in result {
        assert!(close(corner, [1.0, 0.0, 0.0, 1.0]), "{corner:?}");
    }
}

#[test]
fn swapping_u_and_v_turns_the_tangent_to_y_and_flips_the_sign() {
    let uvs = [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];
    let result = tangents(&QUAD_POSITIONS, &QUAD_NORMALS, &uvs, &QUAD_INDICES).expect("tangents");
    for corner in result {
        assert!(close(corner, [0.0, 1.0, 0.0, -1.0]), "{corner:?}");
    }
}

#[test]
fn a_uv_sphere_gets_unit_tangents_perpendicular_to_its_normals() {
    let mesh = Solid::sphere(1.0, 24)
        .expect("sphere")
        .to_mesh()
        .expect("mesh");
    let normals: Vec<[f32; 3]> = mesh
        .positions
        .iter()
        .map(|p| {
            let length = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
            [p[0] / length, p[1] / length, p[2] / length]
        })
        .collect();
    let uvs: Vec<[f32; 2]> = mesh
        .positions
        .iter()
        .map(|p| {
            let u = p[2].atan2(p[0]) / std::f32::consts::TAU + 0.5;
            let v = p[1].clamp(-1.0, 1.0).acos() / std::f32::consts::PI;
            [u, v]
        })
        .collect();
    let result = tangents(&mesh.positions, &normals, &uvs, &mesh.indices).expect("tangents");
    assert_eq!(result.len(), mesh.indices.len());
    let mut good = 0;
    for (corner, index) in result.iter().zip(&mesh.indices) {
        let n = normals[*index as usize];
        let length = (corner[0] * corner[0] + corner[1] * corner[1] + corner[2] * corner[2]).sqrt();
        let along = corner[0] * n[0] + corner[1] * n[1] + corner[2] * n[2];
        assert!(corner.iter().all(|v| v.is_finite()));
        if (length - 1.0).abs() < 1.0e-3
            && along.abs() < 1.0e-3
            && (corner[3].abs() - 1.0).abs() < 1.0e-6
        {
            good += 1;
        }
    }
    // The poles and the UV seam are degenerate in a sphere map; everything else must be exact.
    assert!(
        good * 100 >= result.len() * 95,
        "{good} of {} corners",
        result.len()
    );
}

#[test]
fn the_same_input_gives_bit_identical_output() {
    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let a = tangents(&QUAD_POSITIONS, &QUAD_NORMALS, &uvs, &QUAD_INDICES).expect("first");
    let b = tangents(&QUAD_POSITIONS, &QUAD_NORMALS, &uvs, &QUAD_INDICES).expect("second");
    assert_eq!(a, b);
}

#[test]
fn bad_input_is_refused_with_typed_errors() {
    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    assert!(matches!(
        tangents(&QUAD_POSITIONS, &QUAD_NORMALS[..3], &uvs, &QUAD_INDICES),
        Err(KernelError::InvalidArgument(_))
    ));
    assert!(matches!(
        tangents(&QUAD_POSITIONS, &QUAD_NORMALS, &uvs, &[0, 1, 2, 0, 2]),
        Err(KernelError::InvalidArgument(_))
    ));
    assert!(matches!(
        tangents(&QUAD_POSITIONS, &QUAD_NORMALS, &uvs, &[]),
        Err(KernelError::InvalidArgument(_))
    ));
    assert_eq!(
        tangents(&QUAD_POSITIONS, &QUAD_NORMALS, &uvs, &[0, 1, 9]),
        Err(KernelError::OutOfBounds)
    );
    let mut broken = QUAD_POSITIONS;
    broken[2][0] = f32::NAN;
    assert_eq!(
        tangents(&broken, &QUAD_NORMALS, &uvs, &QUAD_INDICES),
        Err(KernelError::NonFinite)
    );
}
