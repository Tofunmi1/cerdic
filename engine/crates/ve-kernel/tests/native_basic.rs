//! Fixed-value checks for the native basic mesh operators.

use ve_kernel::{MeshOp, MeshSelection, PolygonMesh};

const P: [[f32; 3]; 8] = [
    [-0.5, -0.5, -0.5],
    [0.5, -0.5, -0.5],
    [0.5, 0.5, -0.5],
    [-0.5, 0.5, -0.5],
    [-0.5, -0.5, 0.5],
    [0.5, -0.5, 0.5],
    [0.5, 0.5, 0.5],
    [-0.5, 0.5, 0.5],
];
const C: [u32; 24] = [
    0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6, 3, 0, 4, 7,
];
const S: [u32; 6] = [4; 6];
fn cube() -> PolygonMesh<'static> {
    PolygonMesh {
        positions: &P,
        face_sizes: &S,
        corners: &C,
    }
}
fn volume(p: &[[f32; 3]], sizes: &[u32], corners: &[u32]) -> f64 {
    let mut at = 0;
    let mut total = 0.0;
    for &n in sizes {
        let f = &corners[at..at + n as usize];
        let a = p[f[0] as usize].map(f64::from);
        for i in 1..f.len() - 1 {
            let b = p[f[i] as usize].map(f64::from);
            let c = p[f[i + 1] as usize].map(f64::from);
            total += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0]))
                / 6.0;
        }
        at += n as usize;
    }
    total
}
#[test]
fn native_triangulate_unit_cube() {
    let out =
        ve_kernel::native::basic::edit(&cube(), &MeshSelection::default(), &MeshOp::Triangulate)
            .and_then(Result::ok)
            .expect("native triangulation");
    assert_eq!(out.face_sizes.len(), 12);
    assert!((volume(&out.positions, &out.face_sizes, &out.corners) - 1.0).abs() < 1e-6);
}
#[test]
fn native_recalc_normals_repairs_flipped_cube_face() {
    let mut c = C;
    c[0..4].reverse();
    let m = PolygonMesh {
        positions: &P,
        face_sizes: &S,
        corners: &c,
    };
    let out = ve_kernel::native::basic::edit(
        &m,
        &MeshSelection::default(),
        &MeshOp::RecalcNormals { inside: false },
    )
    .and_then(Result::ok)
    .expect("native recalc");
    assert!((volume(&out.positions, &out.face_sizes, &out.corners) - 1.0).abs() < 1e-6);
}

#[test]
fn native_recalc_normals_inside_makes_cube_volume_negative_one() {
    let out = ve_kernel::native::basic::edit(
        &cube(),
        &MeshSelection::default(),
        &MeshOp::RecalcNormals { inside: true },
    )
    .and_then(Result::ok)
    .expect("native inward normals");
    assert!((volume(&out.positions, &out.face_sizes, &out.corners) + 1.0).abs() < 1e-6);
}

#[test]
fn native_fill_holes_closes_missing_cube_face() {
    let corners: [u32; 20] = [0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6];
    let open = PolygonMesh {
        positions: &P,
        face_sizes: &[4; 5],
        corners: &corners,
    };
    let out = ve_kernel::native::basic::edit(
        &open,
        &MeshSelection::default(),
        &MeshOp::FillHoles { sides: 4 },
    )
    .and_then(Result::ok)
    .expect("native hole fill");
    assert_eq!(out.face_sizes.len(), 6);
    assert!((volume(&out.positions, &out.face_sizes, &out.corners) - 1.0).abs() < 1e-6);
}

mod join {
    use ve_kernel::native::basic::edit;
    use ve_kernel::{MeshOp, MeshSelection, PolygonMesh};

    const POSITIONS: [[f32; 3]; 8] = [
        [-1.0, -1.0, -1.0],
        [1.0, -1.0, -1.0],
        [1.0, 1.0, -1.0],
        [-1.0, 1.0, -1.0],
        [-1.0, -1.0, 1.0],
        [1.0, -1.0, 1.0],
        [1.0, 1.0, 1.0],
        [-1.0, 1.0, 1.0],
    ];
    const QUADS: [u32; 24] = [
        0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6, 3, 0, 4, 7,
    ];

    #[test]
    fn a_triangulated_cube_joins_back_into_six_quads() {
        let mut corners = Vec::new();
        for q in QUADS.chunks(4) {
            corners.extend([q[0], q[1], q[2], q[0], q[2], q[3]]);
        }
        let mesh = PolygonMesh {
            positions: &POSITIONS,
            face_sizes: &[3; 12],
            corners: &corners,
        };
        let op = MeshOp::JoinTriangles {
            face_angle: 0.02,
            shape_angle: 1.3,
        };
        let out = edit(&mesh, &MeshSelection::default(), &op)
            .expect("handled")
            .expect("ok");
        assert_eq!(out.face_sizes.len(), 6);
        assert!(out.face_sizes.iter().all(|&n| n == 4));
    }
}
