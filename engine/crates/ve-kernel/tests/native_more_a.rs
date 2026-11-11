//! Direct native operator checks for this module.

use ve_kernel::{MeshOp, MeshSelection, PolygonMesh};

#[test]
fn spin_is_handled_directly_by_native_module() {
    let positions = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
    ];
    let mesh = PolygonMesh {
        positions: &positions,
        face_sizes: &[4],
        corners: &[0, 1, 2, 3],
    };
    let result = ve_kernel::native::more_a::edit(
        &mesh,
        &MeshSelection::default(),
        &MeshOp::Spin {
            center: [0.0, 0.0, 0.0],
            axis: [0.0, 1.0, 0.0],
            angle: std::f64::consts::TAU,
            steps: 8,
            merge: true,
            distance: 1e-5,
        },
    )
    .expect("spin is native")
    .expect("valid spin");
    assert_eq!(result.face_sizes.len(), 24);
    assert_eq!(result.face_sizes.iter().filter(|&&n| n == 4).count(), 8);
    assert_eq!(result.face_sizes.iter().filter(|&&n| n == 3).count(), 16);
}

#[test]
fn loop_cut_is_handled_directly_by_native_module() {
    let positions = [
        [-0.5, -0.5, -0.5],
        [0.5, -0.5, -0.5],
        [0.5, 0.5, -0.5],
        [-0.5, 0.5, -0.5],
        [-0.5, -0.5, 0.5],
        [0.5, -0.5, 0.5],
        [0.5, 0.5, 0.5],
        [-0.5, 0.5, 0.5],
    ];
    let corners = [
        0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6, 3, 0, 4, 7,
    ];
    let mesh = PolygonMesh {
        positions: &positions,
        face_sizes: &[4; 6],
        corners: &corners,
    };
    let result = ve_kernel::native::more_a::edit(
        &mesh,
        &MeshSelection::default(),
        &MeshOp::LoopCut {
            at: [0.5, 0.0, 0.5],
            cuts: 1,
            smooth: 0.0,
        },
    )
    .expect("loop cut is native")
    .expect("valid ring");
    assert_eq!(result.positions.len(), 12);
    assert_eq!(result.face_sizes.len(), 10);
}
