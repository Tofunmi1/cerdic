//! Edge bevel (Blender's bmesh bevel) through the safe API.

use ve_kernel::{BevelOffset, BevelOptions, BeveledMesh, KernelError, PolygonMesh, bevel_edges};

fn cube() -> (Vec<[f32; 3]>, Vec<u32>, Vec<u32>) {
    let positions = vec![
        [-1.0, -1.0, -1.0],
        [1.0, -1.0, -1.0],
        [1.0, 1.0, -1.0],
        [-1.0, 1.0, -1.0],
        [-1.0, -1.0, 1.0],
        [1.0, -1.0, 1.0],
        [1.0, 1.0, 1.0],
        [-1.0, 1.0, 1.0],
    ];
    let corners = vec![
        0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6, 3, 0, 4, 7,
    ];
    (positions, vec![4; 6], corners)
}

fn options(width: f32, segments: u32) -> BevelOptions {
    BevelOptions {
        width,
        segments,
        offset_type: BevelOffset::Offset,
        profile: 0.5,
    }
}

/// Signed volume of a closed polygon mesh (faces fanned from their first corner).
fn volume(mesh: &BeveledMesh) -> f64 {
    let mut total = 0.0_f64;
    let mut offset = 0;
    for &size in &mesh.face_sizes {
        let corners = &mesh.corners[offset..offset + size as usize];
        let p = |i: u32| mesh.positions[i as usize].map(f64::from);
        let a = p(corners[0]);
        for pair in corners[1..].windows(2) {
            let (b, c) = (p(pair[0]), p(pair[1]));
            total += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0]))
                / 6.0;
        }
        offset += size as usize;
    }
    total
}

#[test]
fn one_edge_bevel_removes_the_closed_form_volume() {
    let (positions, face_sizes, corners) = cube();
    let mesh = PolygonMesh {
        positions: &positions,
        face_sizes: &face_sizes,
        corners: &corners,
    };
    let result = bevel_edges(&mesh, &[[4, 5]], &options(0.25, 3)).unwrap();
    assert_eq!(result.positions.len(), 14);
    assert_eq!(result.face_sizes.len(), 9);
    assert!(
        (volume(&result) - 7.968_75).abs() < 1e-5,
        "{}",
        volume(&result)
    );
}

#[test]
fn every_edge_bevel_stays_a_closed_sphere_topology() {
    let (positions, face_sizes, corners) = cube();
    let mesh = PolygonMesh {
        positions: &positions,
        face_sizes: &face_sizes,
        corners: &corners,
    };
    let edges: Vec<[u32; 2]> = vec![
        [0, 3],
        [3, 2],
        [2, 1],
        [1, 0],
        [4, 5],
        [5, 6],
        [6, 7],
        [7, 4],
        [0, 4],
        [1, 5],
        [2, 6],
        [3, 7],
    ];
    let result = bevel_edges(&mesh, &edges, &options(0.2, 3)).unwrap();
    // Euler: V - E + F = 2 with E = corners / 2 on a closed mesh.
    let euler = result.positions.len() + result.face_sizes.len() - result.corners.len() / 2;
    assert_eq!(euler, 2);
    assert_eq!(result.positions.len(), 96);
    assert_eq!(result.face_sizes.len(), 98);
    // The bevel never moves the bounding box.
    for axis in 0..3 {
        let high = result
            .positions
            .iter()
            .map(|p| p[axis])
            .fold(f32::MIN, f32::max);
        let low = result
            .positions
            .iter()
            .map(|p| p[axis])
            .fold(f32::MAX, f32::min);
        assert!((high - 1.0).abs() < 1e-6 && (low + 1.0).abs() < 1e-6);
    }
}

#[test]
fn provenance_points_at_input_elements() {
    let (positions, face_sizes, corners) = cube();
    let mesh = PolygonMesh {
        positions: &positions,
        face_sizes: &face_sizes,
        corners: &corners,
    };
    let result = bevel_edges(&mesh, &[[4, 5]], &options(0.25, 3)).unwrap();
    assert!(
        result
            .vertex_origin
            .iter()
            .all(|&v| v == BeveledMesh::NO_ORIGIN || (v as usize) < 8)
    );
    assert!(
        result
            .face_parent
            .iter()
            .all(|&f| f == BeveledMesh::NO_ORIGIN || (f as usize) < 6)
    );
    // All six faces survive, so each input face is the parent of at least one output face.
    for face in 0..6_u32 {
        assert!(result.face_parent.contains(&face), "face {face} lost");
    }
}

#[test]
fn bad_input_is_refused_with_a_typed_error() {
    let (positions, face_sizes, corners) = cube();
    let mesh = PolygonMesh {
        positions: &positions,
        face_sizes: &face_sizes,
        corners: &corners,
    };
    // A space diagonal is not an edge.
    assert_eq!(
        bevel_edges(&mesh, &[[0, 6]], &options(0.1, 2)).unwrap_err(),
        KernelError::InvalidArgument("rejected by the kernel")
    );
    assert_eq!(
        bevel_edges(&mesh, &[[0, 99]], &options(0.1, 2)).unwrap_err(),
        KernelError::OutOfBounds
    );
    assert!(bevel_edges(&mesh, &[[0, 3]], &options(-1.0, 2)).is_err());
    assert!(bevel_edges(&mesh, &[[0, 3]], &options(f32::NAN, 2)).is_err());
    assert!(bevel_edges(&mesh, &[[0, 3]], &options(0.1, 0)).is_err());
    assert!(bevel_edges(&mesh, &[[0, 3]], &options(0.1, 65)).is_err());
    assert!(bevel_edges(&mesh, &[], &options(0.1, 2)).is_err());
}

#[test]
fn boundary_edges_are_not_manifold() {
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
    assert_eq!(
        bevel_edges(&mesh, &[[0, 1]], &options(0.1, 2)).unwrap_err(),
        KernelError::NotManifold
    );
}

#[test]
fn malformed_meshes_are_refused() {
    let (mut positions, face_sizes, mut corners) = cube();
    corners[1] = corners[0];
    let mesh = PolygonMesh {
        positions: &positions,
        face_sizes: &face_sizes,
        corners: &corners,
    };
    assert!(bevel_edges(&mesh, &[[0, 3]], &options(0.1, 2)).is_err());
    corners[1] = 3;
    positions[2][1] = f32::INFINITY;
    let mesh = PolygonMesh {
        positions: &positions,
        face_sizes: &face_sizes,
        corners: &corners,
    };
    assert_eq!(
        bevel_edges(&mesh, &[[0, 3]], &options(0.1, 2)).unwrap_err(),
        KernelError::NonFinite
    );
}
