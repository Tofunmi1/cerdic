//! Native extrude, inset, solidify and poke, called directly, with closed-form numbers and the legacy results as a reference.

use std::collections::BTreeMap;

use ve_kernel::native::shape::edit as native;
use ve_kernel::{EditedMesh, MeshOp, MeshSelection, PolygonMesh};

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
// Faces: 0 bottom, 1 top, 2 front (-y), 3 right (+x), 4 back (+y), 5 left (-x).
const CUBE_CORNERS: [u32; 24] = [
    0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6, 3, 0, 4, 7,
];

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

fn volume(mesh: &EditedMesh) -> f64 {
    let mut total = 0.0;
    let mut at = 0;
    for &size in &mesh.face_sizes {
        let c = &mesh.corners[at..at + size as usize];
        let p = |i: u32| mesh.positions[i as usize].map(f64::from);
        let a = p(c[0]);
        for pair in c[1..].windows(2) {
            let (b, d) = (p(pair[0]), p(pair[1]));
            total += (a[0] * (b[1] * d[2] - b[2] * d[1]) - a[1] * (b[0] * d[2] - b[2] * d[0])
                + a[2] * (b[0] * d[1] - b[1] * d[0]))
                / 6.0;
        }
        at += size as usize;
    }
    total
}

fn area(mesh: &EditedMesh, face: usize) -> f64 {
    let start: usize = mesh.face_sizes[..face].iter().map(|&n| n as usize).sum();
    let c = &mesh.corners[start..start + mesh.face_sizes[face] as usize];
    let p = |i: u32| mesh.positions[i as usize].map(f64::from);
    let mut n = [0.0; 3];
    for i in 0..c.len() {
        let (a, b) = (p(c[i]), p(c[(i + 1) % c.len()]));
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() / 2.0
}

fn closed_and_oriented(mesh: &EditedMesh) -> bool {
    let mut directed: BTreeMap<(u32, u32), u32> = BTreeMap::new();
    let mut at = 0;
    for &size in &mesh.face_sizes {
        let f = &mesh.corners[at..at + size as usize];
        for i in 0..f.len() {
            *directed.entry((f[i], f[(i + 1) % f.len()])).or_default() += 1;
        }
        at += size as usize;
    }
    directed
        .iter()
        .all(|(&(a, b), &n)| n == 1 && directed.get(&(b, a)) == Some(&1))
}

fn inset(thickness: f64, depth: f64, even: bool) -> MeshOp {
    MeshOp::Inset {
        thickness,
        depth,
        even,
        relative: false,
        boundary: true,
        outset: false,
    }
}

#[test]
fn inset_of_the_top_leaves_a_ring_and_an_inner_square() {
    let out = native(&cube(), &faces(&[1]), &inset(0.25, 0.0, true))
        .expect("handled")
        .expect("ok");
    assert_eq!(out.face_sizes.len(), 10);
    assert_eq!(out.positions.len(), 12);
    assert!(closed_and_oriented(&out));
    let inner = (0..out.face_sizes.len())
        .find(|&f| out.face_role[f] == 1)
        .expect("inner face");
    assert!(
        (area(&out, inner) - 2.25).abs() < 1e-5,
        "inner side 1.5 -> 2.25, got {}",
        area(&out, inner)
    );
    let ring: f64 = (0..out.face_sizes.len())
        .filter(|&f| out.face_role[f] == 2)
        .map(|f| area(&out, f))
        .sum();
    assert!(
        (ring - 1.75).abs() < 1e-5,
        "ring 4 - 2.25 = 1.75, got {ring}"
    );
    assert!((volume(&out) - 8.0).abs() < 1e-5);
}

#[test]
fn raising_the_inset_adds_the_frustum_volume() {
    let out = native(&cube(), &faces(&[1]), &inset(0.25, 0.5, true))
        .expect("handled")
        .expect("ok");
    // Frustum between the 2x2 base and the 1.5x1.5 top, height 0.5: h/3 * (A + a + sqrt(A a)) = 0.5/3 * (4 + 2.25 + 3).
    let frustum = 0.5 / 3.0 * (4.0 + 2.25 + 3.0);
    assert!(
        (volume(&out) - (8.0 + frustum)).abs() < 1e-4,
        "{}",
        volume(&out)
    );
}

#[test]
fn two_connected_faces_are_inset_as_one_region() {
    // The top and the right face share an edge: the region has 6 border edges, not 8, and shares inner vertices.
    let native_out = native(&cube(), &faces(&[1, 3]), &inset(0.2, 0.0, true))
        .expect("handled")
        .expect("ok");
    // 6 faces keep 4, the 2 selected become inner faces, and 6 border edges get a ring face each: 12 faces. The 8 cube
    // corners plus one inner copy of each of the region's 6 vertices: 14.
    assert_eq!(native_out.face_sizes.len(), 12);
    assert_eq!(native_out.positions.len(), 14);
    assert!(closed_and_oriented(&native_out));
    assert!((volume(&native_out) - 8.0).abs() < 1e-5);
}

#[test]
fn disconnected_faces_are_inset_independently() {
    // Top and bottom share nothing.
    let out = native(&cube(), &faces(&[0, 1]), &inset(0.2, 0.0, false))
        .expect("handled")
        .expect("ok");
    assert_eq!(out.face_sizes.len(), 6 + 8);
    assert_eq!(out.positions.len(), 8 + 8);
    assert!(closed_and_oriented(&out));
}

#[test]
fn even_insets_keep_the_ring_width_and_uneven_ones_shrink_corners() {
    let even = native(&cube(), &faces(&[1]), &inset(0.2, 0.0, true))
        .expect("handled")
        .expect("ok");
    let plain = native(&cube(), &faces(&[1]), &inset(0.2, 0.0, false))
        .expect("handled")
        .expect("ok");
    let side = |m: &EditedMesh| {
        let f = (0..m.face_sizes.len())
            .find(|&f| m.face_role[f] == 1)
            .expect("inner");
        area(m, f).sqrt()
    };
    assert!(
        (side(&even) - 1.6).abs() < 1e-5,
        "even: inner side 2 - 2*0.2"
    );
    assert!(
        side(&plain) > 1.6 && side(&plain) < 2.0,
        "uneven corners move less: {}",
        side(&plain)
    );
}

#[test]
fn outset_grows_the_face_outward() {
    let op = MeshOp::Inset {
        thickness: 0.25,
        depth: 0.0,
        even: true,
        relative: false,
        boundary: true,
        outset: true,
    };
    let out = native(&cube(), &faces(&[1]), &op)
        .expect("handled")
        .expect("ok");
    let inner = (0..out.face_sizes.len())
        .find(|&f| out.face_role[f] == 1)
        .expect("inner");
    assert!((area(&out, inner) - 6.25).abs() < 1e-5, "side 2.5 -> 6.25");
}

#[test]
fn extrude_of_two_disconnected_regions() {
    let op = MeshOp::Extrude {
        offset: [0.0, 0.0, 0.5],
    };
    let out = native(&cube(), &faces(&[1]), &op)
        .expect("handled")
        .expect("ok");
    assert!((volume(&out) - 10.0).abs() < 1e-5);
    assert_eq!(out.face_sizes.len(), 10);
    assert!(closed_and_oriented(&out));
    // Top and right face together: the shared edge gets no wall.
    let bent = native(
        &cube(),
        &faces(&[1, 3]),
        &MeshOp::Extrude {
            offset: [0.0, 0.0, 0.0],
        },
    )
    .expect("handled")
    .expect("ok");
    assert_eq!(bent.face_sizes.len(), 6 + 6);
    assert!(closed_and_oriented(&bent));
}

#[test]
fn solidify_a_single_square_and_a_closed_box() {
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
    let out = native(&quad, &faces(&[]), &MeshOp::Solidify { thickness: 0.1 })
        .expect("handled")
        .expect("ok");
    assert_eq!(out.positions.len(), 8);
    assert_eq!(out.face_sizes.len(), 6);
    assert!((volume(&out).abs() - 0.1).abs() < 1e-6);
    assert!(closed_and_oriented(&out));
    // A closed box: an inner shell 0.1 inside. Outer volume 8, inner box (2 - 0.2)^3 = 5.832, so the wall is 2.168.
    let inward = native(&cube(), &faces(&[]), &MeshOp::Solidify { thickness: -0.1 })
        .expect("handled")
        .expect("ok");
    assert!(
        (volume(&inward).abs() - (8.0 - 5.832)).abs() < 1e-3,
        "{}",
        volume(&inward)
    );
}

#[test]
fn poke_raises_the_closed_form_pyramid() {
    let op = MeshOp::Poke {
        offset: 0.5,
        center: ve_kernel::PokeCenter::Median,
        relative: false,
    };
    let out = native(&cube(), &faces(&[1]), &op)
        .expect("handled")
        .expect("ok");
    assert_eq!(out.face_sizes.len(), 9);
    assert!((volume(&out) - (8.0 + 4.0 * 0.5 / 3.0)).abs() < 1e-5);
}

#[test]
fn unimplemented_operators_decline() {
    let op = MeshOp::Subdivide {
        cuts: 1,
        smooth: 0.0,
        grid_fill: false,
    };
    assert!(native(&cube(), &faces(&[]), &op).is_none());
}
