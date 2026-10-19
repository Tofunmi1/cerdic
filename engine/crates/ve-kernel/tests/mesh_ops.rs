//! Blender's bmesh operators (inset, solidify, extrude, dissolve, poke, subdivide, wireframe,
//! symmetrize) through the safe API, checked against closed-form volumes and element counts.

use ve_kernel::{
    EditedMesh, KernelError, MeshOp, MeshSelection, PokeCenter, PolygonMesh, edit_mesh,
};

const CUBE_POSITIONS: [[f32; 3]; 8] = [
    [-1.0, -1.0, -1.0],
    [1.0, -1.0, -1.0],
    [1.0, 1.0, -1.0],
    [-1.0, 1.0, -1.0],
    [-1.0, -1.0, 1.0],
    [1.0, -1.0, 1.0],
    [1.0, 1.0, 1.0],
    [-1.0, 1.0, 1.0],
];
// Faces: 0 bottom (-z), 1 top (+z), 2 front (-y), 3 right (+x), 4 back (+y), 5 left (-x).
const CUBE_CORNERS: [u32; 24] = [
    0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6, 3, 0, 4, 7,
];
const TOP: u32 = 1;

fn cube() -> PolygonMesh<'static> {
    PolygonMesh {
        positions: &CUBE_POSITIONS,
        face_sizes: &[4; 6],
        corners: &CUBE_CORNERS,
    }
}

fn faces(list: &[u32]) -> MeshSelection<'_> {
    MeshSelection {
        faces: list,
        edges: &[],
        vertices: &[],
    }
}

/// Signed volume of a closed polygon mesh.
fn volume(mesh: &EditedMesh) -> f64 {
    let mut total = 0.0_f64;
    let mut offset = 0;
    let p = |i: u32| mesh.positions[i as usize].map(f64::from);
    for &size in &mesh.face_sizes {
        let corners = &mesh.corners[offset..offset + size as usize];
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

/// Vertices - edges + faces for a mesh where every edge has two corners.
fn euler(mesh: &EditedMesh) -> i64 {
    let count = |n: usize| i64::try_from(n).unwrap_or(i64::MAX);
    count(mesh.positions.len()) - count(mesh.corners.len() / 2) + count(mesh.face_sizes.len())
}

#[test]
fn inset_adds_a_ring_and_raising_it_adds_the_frustum_volume() {
    let flat = MeshOp::Inset {
        thickness: 0.2,
        depth: 0.0,
        even: false,
        relative: false,
        boundary: true,
        outset: false,
    };
    let result = edit_mesh(&cube(), &faces(&[TOP]), &flat).unwrap();
    assert_eq!(result.face_sizes.len(), 10);
    assert_eq!(result.positions.len(), 12);
    assert!((volume(&result) - 8.0).abs() < 1e-5);
    let raised = MeshOp::Inset {
        thickness: 0.2,
        depth: 0.1,
        even: false,
        relative: false,
        boundary: true,
        outset: false,
    };
    let result = edit_mesh(&cube(), &faces(&[TOP]), &raised).unwrap();
    // The inner face moves 0.1 up along its normal, so four vertices sit at z = 1.1 and the cube gains a
    // truncated pyramid: more than a flat ring (8) and less than a full 2 x 2 x 0.1 slab (8.4).
    let lifted = result
        .positions
        .iter()
        .filter(|p| (p[2] - 1.1).abs() < 1e-6)
        .count();
    assert_eq!(lifted, 4);
    assert!(
        volume(&result) > 8.2 && volume(&result) < 8.4,
        "{}",
        volume(&result)
    );
}

#[test]
fn extrude_moves_the_new_faces_by_the_offset() {
    let op = MeshOp::Extrude {
        offset: [0.0, 0.0, 0.5],
    };
    let result = edit_mesh(&cube(), &faces(&[TOP]), &op).unwrap();
    // The 2 x 2 top pushed up by 0.5 adds a 4 x 0.5 slab.
    assert!((volume(&result) - 10.0).abs() < 1e-5, "{}", volume(&result));
    assert_eq!(euler(&result), 2);
}

#[test]
fn poke_raises_a_pyramid_on_the_face() {
    let op = MeshOp::Poke {
        offset: 0.5,
        center: PokeCenter::Median,
        relative: false,
    };
    let result = edit_mesh(&cube(), &faces(&[TOP]), &op).unwrap();
    assert_eq!(result.face_sizes.len(), 6 - 1 + 4);
    assert!((volume(&result) - (8.0 + 4.0 * 0.5 / 3.0)).abs() < 1e-5);
}

#[test]
fn solidify_gives_a_single_face_its_thickness() {
    let positions = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
    ];
    let quad = PolygonMesh {
        positions: &positions,
        face_sizes: &[4],
        corners: &[0, 1, 2, 3],
    };
    let result = edit_mesh(&quad, &faces(&[]), &MeshOp::Solidify { thickness: 0.1 }).unwrap();
    assert_eq!(result.positions.len(), 8);
    assert_eq!(result.face_sizes.len(), 6);
    assert!(
        (volume(&result).abs() - 0.1).abs() < 1e-5,
        "{}",
        volume(&result)
    );
}

#[test]
fn dissolve_merges_the_coplanar_triangles_of_a_triangulated_cube() {
    // Each face split along a diagonal: 12 triangles.
    let mut corners = Vec::new();
    for quad in CUBE_CORNERS.chunks(4) {
        corners.extend_from_slice(&[quad[0], quad[1], quad[2], quad[0], quad[2], quad[3]]);
    }
    let mesh = PolygonMesh {
        positions: &CUBE_POSITIONS,
        face_sizes: &[3; 12],
        corners: &corners,
    };
    let op = MeshOp::Dissolve {
        angle: 0.01,
        boundaries: false,
    };
    let result = edit_mesh(&mesh, &faces(&[]), &op).unwrap();
    assert_eq!(result.face_sizes.len(), 6);
    assert!(result.face_sizes.iter().all(|&n| n == 4));
    assert!((volume(&result) - 8.0).abs() < 1e-5);
}

#[test]
fn triangulate_splits_cube_quads_into_twelve_triangles() {
    let result = edit_mesh(&cube(), &MeshSelection::default(), &MeshOp::Triangulate).unwrap();
    assert_eq!(result.face_sizes.len(), 12);
    assert!(result.face_sizes.iter().all(|&size| size == 3));
    assert_eq!(euler(&result), 2);
    assert!((volume(&result) - 8.0).abs() < 1e-5);
}

#[test]
fn remove_doubles_merges_only_selected_nearby_vertices() {
    let positions = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 0.0, 0.0],
        [2.0, 0.0, 0.0],
        [2.0, 1.0, 0.0],
        [1.0, 1.0, 0.0],
    ];
    let mesh = PolygonMesh {
        positions: &positions,
        face_sizes: &[4, 4],
        corners: &[0, 1, 2, 3, 4, 5, 6, 7],
    };
    let selection = MeshSelection {
        faces: &[],
        edges: &[],
        vertices: &[1, 2, 4, 7],
    };
    let result = edit_mesh(
        &mesh,
        &selection,
        &MeshOp::RemoveDoubles { distance: 1.0e-4 },
    )
    .unwrap();
    assert_eq!(result.positions.len(), 6);
    assert_eq!(result.face_sizes, [4, 4]);
}

#[test]
fn subdividing_an_edge_keeps_the_surface_closed_and_the_volume() {
    let selection = MeshSelection {
        faces: &[],
        edges: &[[4, 5]],
        vertices: &[],
    };
    let op = MeshOp::Subdivide {
        cuts: 1,
        smooth: 0.0,
        grid_fill: false,
    };
    let result = edit_mesh(&cube(), &selection, &op).unwrap();
    assert_eq!(result.positions.len(), 9);
    assert_eq!(euler(&result), 2);
    assert!((volume(&result) - 8.0).abs() < 1e-5);
}

#[test]
fn wireframe_turns_a_face_into_struts() {
    let positions = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
    ];
    let quad = PolygonMesh {
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
    let result = edit_mesh(&quad, &faces(&[]), &op).unwrap();
    assert!(
        result.face_sizes.len() > 4,
        "{} faces",
        result.face_sizes.len()
    );
}

#[test]
fn symmetrize_mirrors_half_a_box_into_a_whole_one() {
    // A box x in [0, 1], y and z in [-0.5, 0.5], open at the x = 0 side.
    let positions = [
        [0.0, -0.5, -0.5],
        [1.0, -0.5, -0.5],
        [1.0, 0.5, -0.5],
        [0.0, 0.5, -0.5],
        [0.0, -0.5, 0.5],
        [1.0, -0.5, 0.5],
        [1.0, 0.5, 0.5],
        [0.0, 0.5, 0.5],
    ];
    let corners = [0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6];
    let half = PolygonMesh {
        positions: &positions,
        face_sizes: &[4; 5],
        corners: &corners,
    };
    let op = MeshOp::Symmetrize {
        axis: 0,
        keep_positive: true,
        distance: 1e-4,
    };
    let result = edit_mesh(&half, &faces(&[]), &op).unwrap();
    assert_eq!(result.positions.len(), 12);
    assert_eq!(result.face_sizes.len(), 10);
    assert!(
        (volume(&result).abs() - 2.0).abs() < 1e-5,
        "{}",
        volume(&result)
    );
}

/// A bumpy `n` by `n` grid of quads split into triangles.
fn bumpy_grid(n: u32) -> (Vec<[f32; 3]>, Vec<u32>) {
    let mut positions = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            let (x, y) = (i as f32 / n as f32, j as f32 / n as f32);
            positions.push([x, y, 0.05 * (6.0 * x).sin() * (5.0 * y).cos()]);
        }
    }
    let mut corners = Vec::new();
    for j in 0..n {
        for i in 0..n {
            let a = j * (n + 1) + i;
            let (b, c, d) = (a + 1, a + n + 2, a + n + 1);
            corners.extend_from_slice(&[a, b, c, a, c, d]);
        }
    }
    (positions, corners)
}

#[test]
fn decimate_removes_faces_and_unsubdivide_a_regular_grid() {
    let (positions, corners) = bumpy_grid(24);
    let faces = corners.len() / 3;
    let mesh = PolygonMesh {
        positions: &positions,
        face_sizes: &vec![3; faces],
        corners: &corners,
    };
    let op = MeshOp::Decimate {
        ratio: 0.25,
        symmetry: None,
        triangulate: true,
    };
    let result = edit_mesh(&mesh, &MeshSelection::default(), &op).unwrap();
    let kept = result.face_sizes.len() as f64 / faces as f64;
    assert!((0.2..0.35).contains(&kept), "kept {kept} of {faces}");
    // Every surviving vertex came from an input vertex.
    assert!(
        result
            .vertex_origin
            .iter()
            .all(|&v| (v as usize) < positions.len())
    );
}

#[test]
fn bad_requests_are_refused_with_typed_errors() {
    let top = faces(&[TOP]);
    let inset = |thickness| MeshOp::Inset {
        thickness,
        depth: 0.0,
        even: false,
        relative: false,
        boundary: true,
        outset: false,
    };
    assert_eq!(
        edit_mesh(&cube(), &top, &inset(-0.1)).unwrap_err(),
        KernelError::InvalidArgument("rejected by the kernel")
    );
    assert_eq!(
        edit_mesh(&cube(), &top, &inset(f64::NAN)).unwrap_err(),
        KernelError::NonFinite
    );
    // Inset needs faces; a face index past the mesh is out of bounds.
    assert!(edit_mesh(&cube(), &faces(&[]), &inset(0.1)).is_err());
    assert_eq!(
        edit_mesh(&cube(), &faces(&[99]), &inset(0.1)).unwrap_err(),
        KernelError::OutOfBounds
    );
    // Subdivide needs edges that exist.
    let missing = MeshSelection {
        faces: &[],
        edges: &[[0, 6]],
        vertices: &[],
    };
    let cut = MeshOp::Subdivide {
        cuts: 1,
        smooth: 0.0,
        grid_fill: false,
    };
    assert!(edit_mesh(&cube(), &missing, &cut).is_err());
    let too_many = MeshOp::Subdivide {
        cuts: 9,
        smooth: 0.0,
        grid_fill: false,
    };
    let edge = MeshSelection {
        faces: &[],
        edges: &[[4, 5]],
        vertices: &[],
    };
    assert!(edit_mesh(&cube(), &edge, &too_many).is_err());
    assert!(edit_mesh(&cube(), &faces(&[]), &MeshOp::Solidify { thickness: 0.0 }).is_err());
}
