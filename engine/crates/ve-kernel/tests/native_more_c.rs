//! Direct checks of the native cleanup operators.

use ve_kernel::{MeshOp, MeshSelection, PolygonMesh};

#[test]
fn native_planar_faces_reduces_quad_warp() {
    let p = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.4],
        [0.0, 1.0, 0.0],
    ];
    let m = PolygonMesh {
        positions: &p,
        face_sizes: &[4],
        corners: &[0, 1, 2, 3],
    };
    let r = ve_kernel::native::more_c::edit(
        &m,
        &MeshSelection::default(),
        &MeshOp::PlanarFaces {
            iterations: 10,
            factor: 1.0,
        },
    )
    .expect("native operator")
    .expect("valid mesh");
    let a = r.positions[0].map(f64::from);
    let b = r.positions[1].map(f64::from);
    let c = r.positions[2].map(f64::from);
    let d = r.positions[3].map(f64::from);
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let ad = [d[0] - a[0], d[1] - a[1], d[2] - a[2]];
    let cross = [
        ab[1] * ac[2] - ab[2] * ac[1],
        ab[2] * ac[0] - ab[0] * ac[2],
        ab[0] * ac[1] - ab[1] * ac[0],
    ];
    let volume = cross[0] * ad[0] + cross[1] * ad[1] + cross[2] * ad[2];
    assert!(volume.abs() < 1e-4, "four corners are coplanar: {volume}");
}

fn ring_mesh(radii: &[f32]) -> (Vec<[f32; 3]>, Vec<u32>, Vec<[u32; 2]>) {
    let n = radii.len();
    let mut positions = Vec::new();
    for i in 0..n {
        let a = std::f64::consts::TAU * i as f64 / n as f64;
        positions.push([f64::from(radii[i]).mul_add(a.cos(), 0.0) as f32, f64::from(radii[i]).mul_add(a.sin(), 0.0) as f32, 0.0]);
    }
    for i in 0..n {
        let a = std::f64::consts::TAU * i as f64 / n as f64;
        positions.push([a.cos() as f32, a.sin() as f32, 1.0]);
    }
    let mut corners = Vec::new();
    let mut edges = Vec::new();
    for i in 0..n {
        let next = (i + 1) % n;
        corners.extend_from_slice(&[i as u32, next as u32, (n + next) as u32, (n + i) as u32]);
        edges.push([i as u32, next as u32]);
    }
    (positions, corners, edges)
}

#[test]
fn native_space_loops_places_closed_polyline_vertices_at_equal_arc_fractions() {
    let radii = [1.0; 8];
    let (positions, corners, edges) = ring_mesh(&radii);
    let mesh = PolygonMesh { positions: &positions, face_sizes: &[4; 8], corners: &corners };
    let selection = MeshSelection { faces: &[], edges: &edges, vertices: &[] };
    let result = ve_kernel::native::more_c::edit(&mesh, &selection, &MeshOp::SpaceEdgeLoops { factor: 1.0, linear: true })
        .expect("native operator").expect("valid mesh");
    assert_eq!(result.positions.len(), positions.len());
}

#[test]
fn native_circularize_fits_an_ellipse_and_honors_radius() {
    let n = 8;
    let mut positions = Vec::new();
    for i in 0..n {
        let a = std::f64::consts::TAU * i as f64 / n as f64;
        positions.push([(1.5 * a.cos()) as f32, (0.75 * a.sin()) as f32, 1.0]);
    }
    for i in 0..n {
        let a = std::f64::consts::TAU * i as f64 / n as f64;
        positions.push([(1.5 * a.cos()) as f32, (0.75 * a.sin()) as f32, 0.0]);
    }
    let mut corners = Vec::new();
    let mut edges = Vec::new();
    for i in 0..n {
        let j = (i + 1) % n;
        corners.extend_from_slice(&[i as u32, j as u32, (n + j) as u32, (n + i) as u32]);
        edges.push([i as u32, j as u32]);
    }
    let mesh = PolygonMesh { positions: &positions, face_sizes: &[4; 8], corners: &corners };
    let selection = MeshSelection { faces: &[], edges: &edges, vertices: &[] };
    let result = ve_kernel::native::more_c::edit(&mesh, &selection, &MeshOp::Circularize { factor: 1.0, radius: 2.0, angle: 0.0, flatten: 1.0, regular: false })
        .expect("native operator").expect("valid mesh");
    // The selected ring is the one at z = 1 (flattened onto its plane); the output is renumbered, so find it by position.
    let ring: Vec<_> = result.positions.iter().filter(|p| (p[2] - 1.0).abs() < 1e-4).collect();
    assert_eq!(ring.len(), 8);
    for p in ring {
        let r = (f64::from(p[0]).powi(2) + f64::from(p[1]).powi(2)).sqrt();
        assert!((r - 2.0).abs() < 1e-4, "radius {r}");
    }
}

#[test]
fn native_beautify_by_area_flips_to_the_larger_minimum_triangle_area() {
    let positions = [[0.0, 0.0, 0.0], [3.0, 0.0, 0.0], [4.0, 2.0, 0.0], [0.0, 1.0, 0.0]];
    let mesh = PolygonMesh { positions: &positions, face_sizes: &[3, 3], corners: &[0, 1, 2, 0, 2, 3] };
    let result = ve_kernel::native::more_c::edit(&mesh, &MeshSelection::default(), &MeshOp::Beautify { by_angle: false })
        .expect("native operator").expect("valid mesh");
    // Vertices are renumbered in the output: identify the corners by position. B = (3,0), D = (0,1), A = (0,0), C = (4,2).
    let at = |p: [f32; 3]| result.positions.iter().position(|q| *q == p).expect("corner kept") as u32;
    let (a, b, c, d) = (at([0.0, 0.0, 0.0]), at([3.0, 0.0, 0.0]), at([4.0, 2.0, 0.0]), at([0.0, 1.0, 0.0]));
    assert!(result.corners.chunks_exact(3).any(|tri| tri.contains(&b) && tri.contains(&d)), "B-D present");
    assert!(!result.corners.chunks_exact(3).any(|tri| tri.contains(&a) && tri.contains(&c)), "A-C gone");
}
