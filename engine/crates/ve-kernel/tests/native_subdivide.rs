//! Native edge subdivision on a cube: closed, volume-preserving, with the face counts the patterns imply.

use std::collections::BTreeMap;

use ve_kernel::native::subdivide::edit;
use ve_kernel::{EditedMesh, MeshOp, MeshSelection, PolygonMesh};

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
const CORNERS: [u32; 24] = [0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6, 3, 0, 4, 7];

fn cube() -> PolygonMesh<'static> {
    PolygonMesh { positions: &POSITIONS, face_sizes: &[4; 6], corners: &CORNERS }
}

fn cut(edges: &[[u32; 2]], cuts: u32, grid_fill: bool) -> EditedMesh {
    let selection = MeshSelection { faces: &[], edges, vertices: &[] };
    edit(&cube(), &selection, &MeshOp::Subdivide { cuts, smooth: 0.0, grid_fill })
        .expect("handled")
        .expect("ok")
}

fn volume(m: &EditedMesh) -> f64 {
    let mut total = 0.0;
    let mut at = 0;
    for &size in &m.face_sizes {
        let c = &m.corners[at..at + size as usize];
        let p = |i: u32| m.positions[i as usize].map(f64::from);
        let a = p(c[0]);
        for pair in c[1..].windows(2) {
            let (b, d) = (p(pair[0]), p(pair[1]));
            total += (a[0] * (b[1] * d[2] - b[2] * d[1]) - a[1] * (b[0] * d[2] - b[2] * d[0]) + a[2] * (b[0] * d[1] - b[1] * d[0])) / 6.0;
        }
        at += size as usize;
    }
    total
}

fn closed_and_oriented(m: &EditedMesh) -> bool {
    let mut directed: BTreeMap<(u32, u32), u32> = BTreeMap::new();
    let mut at = 0;
    for &size in &m.face_sizes {
        let f = &m.corners[at..at + size as usize];
        for i in 0..f.len() {
            *directed.entry((f[i], f[(i + 1) % f.len()])).or_default() += 1;
        }
        at += size as usize;
    }
    directed.iter().all(|(&(a, b), &n)| n == 1 && directed.get(&(b, a)) == Some(&1))
}

#[test]
fn a_single_edge_cut_three_times() {
    let out = cut(&[[4, 5]], 3, false);
    assert_eq!(out.positions.len(), 8 + 3);
    assert!(closed_and_oriented(&out));
    assert!((volume(&out) - 8.0).abs() < 1e-5);
}

#[test]
fn four_parallel_edges_turn_the_side_faces_into_strips() {
    // The vertical edges 0-4, 1-5, 2-6 and 3-7 cut once: each of the four side faces has two opposite cut edges.
    let out = cut(&[[0, 4], [1, 5], [2, 6], [3, 7]], 1, false);
    assert_eq!(out.positions.len(), 8 + 4);
    assert_eq!(out.face_sizes.len(), 2 + 4 * 2);
    assert!(out.face_sizes.iter().all(|&n| n == 4 || n == 6), "{:?}", out.face_sizes);
    assert!(closed_and_oriented(&out));
    assert!((volume(&out) - 8.0).abs() < 1e-5);
}

#[test]
fn every_edge_with_grid_fill_makes_a_grid_on_each_face() {
    let all = [[0, 3], [3, 2], [2, 1], [1, 0], [4, 5], [5, 6], [6, 7], [7, 4], [0, 4], [1, 5], [2, 6], [3, 7]];
    let out = cut(&all, 1, true);
    // 8 corners + 12 edge points + 6 face points; each face becomes 2 x 2 quads.
    assert_eq!(out.positions.len(), 26);
    assert_eq!(out.face_sizes.len(), 24);
    assert!(out.face_sizes.iter().all(|&n| n == 4));
    assert!(closed_and_oriented(&out));
    assert!((volume(&out) - 8.0).abs() < 1e-5);
    let triple = cut(&all, 2, true);
    assert_eq!(triple.face_sizes.len(), 6 * 9);
    assert!(closed_and_oriented(&triple));
}

#[test]
fn smoothing_and_bad_requests_are_handled() {
    let selection = MeshSelection { faces: &[], edges: &[[4, 5]], vertices: &[] };
    assert!(edit(&cube(), &selection, &MeshOp::Subdivide { cuts: 1, smooth: 1.5, grid_fill: false }).expect("handled").is_err(), "smooth above one is refused");
    let missing = MeshSelection { faces: &[], edges: &[[0, 6]], vertices: &[] };
    assert!(edit(&cube(), &missing, &MeshOp::Subdivide { cuts: 1, smooth: 0.0, grid_fill: false }).expect("handled").is_err());
    assert!(edit(&cube(), &selection, &MeshOp::Subdivide { cuts: 9, smooth: 0.0, grid_fill: false }).expect("handled").is_err());
    assert!(edit(&cube(), &MeshSelection::default(), &MeshOp::Subdivide { cuts: 1, smooth: 0.0, grid_fill: false }).expect("handled").is_err());
}

#[test]
fn smoothing_pushes_cut_points_out_along_the_arc_and_keeps_the_surface_closed() {
    let selection = MeshSelection { faces: &[], edges: &[[4, 5]], vertices: &[] };
    let flat = edit(&cube(), &selection, &MeshOp::Subdivide { cuts: 1, smooth: 0.0, grid_fill: false }).expect("handled").expect("ok");
    let round = edit(&cube(), &selection, &MeshOp::Subdivide { cuts: 1, smooth: 1.0, grid_fill: false }).expect("handled").expect("ok");
    assert_eq!(flat.positions.len(), round.positions.len());
    assert!(closed_and_oriented(&round));
    // The new point sits at the middle of the top-front edge (0, -1, 1). The faces meet at 90 degrees, so the arc's
    // height is (L / 2) * tan(90 / 4 degrees) = 1 * 0.41421, along the diagonal (0, -1, 1) / sqrt 2.
    let new = round.positions.iter().find(|p| !POSITIONS.contains(p)).expect("one new point");
    let height = (std::f64::consts::FRAC_PI_8).tan();
    let expected = [0.0, -1.0 - height / 2.0_f64.sqrt(), 1.0 + height / 2.0_f64.sqrt()];
    for k in 0..3 {
        assert!((f64::from(new[k]) - expected[k]).abs() < 1e-5, "axis {k}: {new:?} vs {expected:?}");
    }
    // Half the smoothing, half the push.
    let half = edit(&cube(), &selection, &MeshOp::Subdivide { cuts: 1, smooth: 0.5, grid_fill: false }).expect("handled").expect("ok");
    let mid = half.positions.iter().find(|p| !POSITIONS.contains(p)).expect("one new point");
    assert!((f64::from(mid[2]) - (1.0 + height / 2.0_f64.sqrt() / 2.0)).abs() < 1e-5);
    assert!(volume(&round) > 8.0, "the cut point moved outward");
}
