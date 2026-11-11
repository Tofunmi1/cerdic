//! Fixed-value checks for native mesh operators owned by helper F.

use ve_kernel::{MeshOp, MeshSelection, PolygonMesh};

#[test]
fn native_wireframe_unit_quad_frame_area() {
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
    let op = MeshOp::Wireframe {
        thickness: 0.1,
        offset: 0.0,
        even: true,
        relative: false,
        boundary: true,
        replace: true,
    };
    let out = ve_kernel::native::more_b::edit(&mesh, &MeshSelection::default(), &op)
        .and_then(Result::ok)
        .expect("native wireframe");
    assert_eq!(out.face_sizes.len(), 16);
    assert_eq!(out.positions.len(), 16);
    let mut volume = 0.0_f64;
    let mut at = 0;
    for &size in &out.face_sizes {
        let f = &out.corners[at..at + size as usize];
        let a = out.positions[f[0] as usize].map(f64::from);
        for tri in f[1..].windows(2) {
            let b = out.positions[tri[0] as usize].map(f64::from);
            let c = out.positions[tri[1] as usize].map(f64::from);
            volume += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0]))
                / 6.0;
        }
        at += size as usize;
    }
    assert!((volume.abs() - 0.036).abs() < 1e-6, "volume={volume}");
}

#[test]
fn native_rotate_selected_triangle_edge() {
    let positions = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
    ];
    let mesh = PolygonMesh {
        positions: &positions,
        face_sizes: &[3, 3],
        corners: &[0, 1, 2, 0, 2, 3],
    };
    let selection = MeshSelection {
        edges: &[[0, 2]],
        ..MeshSelection::default()
    };
    let out = ve_kernel::native::more_b::edit(
        &mesh,
        &selection,
        &MeshOp::RotateEdges {
            counter_clockwise: false,
        },
    )
    .and_then(Result::ok)
    .expect("native edge rotation");
    assert_eq!(out.face_sizes, vec![3, 3]);
    let has_old_diagonal = out.corners.chunks_exact(3).any(|face| {
        let has_a = face
            .iter()
            .any(|&v| out.positions[v as usize] == positions[0]);
        let has_c = face
            .iter()
            .any(|&v| out.positions[v as usize] == positions[2]);
        has_a && has_c
    });
    assert!(!has_old_diagonal);
}

#[test]
fn native_connect_opposite_quad_vertices_splits_face() {
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
    let selection = MeshSelection {
        vertices: &[0, 2],
        ..MeshSelection::default()
    };
    let out = ve_kernel::native::more_b::edit(
        &mesh,
        &selection,
        &MeshOp::ConnectVertices {
            check_degenerate: true,
        },
    )
    .and_then(Result::ok)
    .expect("native vertex connection");
    assert_eq!(out.face_sizes, vec![3, 3]);
}


#[test]
fn wireframe_without_the_boundary_drops_the_outer_walls_of_open_edges() {
    let positions = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.0, 2.0, 0.0], [0.0, 2.0, 0.0]];
    let mesh = PolygonMesh { positions: &positions, face_sizes: &[4], corners: &[0, 1, 2, 3] };
    let run = |boundary| {
        ve_kernel::native::more_b::edit(
            &mesh,
            &MeshSelection::default(),
            &MeshOp::Wireframe { thickness: 0.1, offset: 0.0, even: true, relative: false, boundary, replace: true },
        )
        .expect("native wireframe")
        .expect("ok")
    };
    let (with, without) = (run(true), run(false));
    assert_eq!(with.face_sizes.len(), 16);
    // A single quad has four open edges, so all four outer walls go.
    assert_eq!(without.face_sizes.len(), 12);
}
