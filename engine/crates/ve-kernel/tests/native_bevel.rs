//! The native edge bevel, called directly and checked against closed-form volumes, counts and structure.

use std::collections::BTreeMap;

use ve_kernel::native::bevel::bevel_edges as native;
use ve_kernel::{BevelOffset, BevelOptions, BeveledMesh, PolygonMesh};

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
const CUBE_CORNERS: [u32; 24] = [
    0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6, 3, 0, 4, 7,
];
const ALL_EDGES: [[u32; 2]; 12] = [
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

fn cube() -> PolygonMesh<'static> {
    PolygonMesh {
        positions: &CUBE_POSITIONS,
        face_sizes: &[4; 6],
        corners: &CUBE_CORNERS,
    }
}

fn opts(width: f32, segments: u32) -> BevelOptions {
    BevelOptions {
        width,
        segments,
        offset_type: BevelOffset::Offset,
        profile: 0.5,
    }
}

fn volume(mesh: &BeveledMesh) -> f64 {
    let mut total = 0.0_f64;
    let mut at = 0;
    for &size in &mesh.face_sizes {
        let corners = &mesh.corners[at..at + size as usize];
        let p = |i: u32| mesh.positions[i as usize].map(f64::from);
        let a = p(corners[0]);
        for pair in corners[1..].windows(2) {
            let (b, c) = (p(pair[0]), p(pair[1]));
            total += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0]))
                / 6.0;
        }
        at += size as usize;
    }
    total
}

/// Every undirected edge has exactly two faces and every directed edge appears once.
fn closed_and_oriented(mesh: &BeveledMesh) -> bool {
    let mut directed: BTreeMap<(u32, u32), u32> = BTreeMap::new();
    let mut at = 0;
    for &size in &mesh.face_sizes {
        let face = &mesh.corners[at..at + size as usize];
        for i in 0..face.len() {
            *directed
                .entry((face[i], face[(i + 1) % face.len()]))
                .or_default() += 1;
        }
        at += size as usize;
    }
    directed
        .iter()
        .all(|(&(a, b), &n)| n == 1 && directed.get(&(b, a)) == Some(&1))
}

fn euler(mesh: &BeveledMesh) -> i64 {
    let v = mesh.positions.len() as i64;
    let f = mesh.face_sizes.len() as i64;
    let e = mesh.corners.len() as i64 / 2;
    v - e + f
}

fn compare(edges: &[[u32; 2]], options: &BevelOptions, tolerance: f64) {
    let ours = native(&cube(), edges, options)
        .expect("native handles it")
        .expect("native result");
    assert!(
        closed_and_oriented(&ours),
        "native result must be a closed, consistently wound mesh"
    );
    assert_eq!(euler(&ours), 2);
    // A cube of side 2 with every edge rounded by radius r has the volume of its rounded box (Steiner):
    // a^3 + 6 a^2 r + 3 pi a r^2 + 4/3 pi r^3 with a = 2 - 2r. The polygonal profile stays just inside it.
    let r = f64::from(options.width);
    let a = 2.0 - 2.0 * r;
    let exact = a.powi(3) + 6.0 * a * a * r + 3.0 * std::f64::consts::PI * a * r * r
        + 4.0 / 3.0 * std::f64::consts::PI * r.powi(3);
    let v = volume(&ours);
    assert!(v <= exact * (1.0 + 1e-6) && v >= exact * (1.0 - tolerance), "volume {v} vs rounded box {exact}");
}

#[test]
fn one_edge_matches_the_closed_form_volume() {
    let ours = native(&cube(), &[[4, 5]], &opts(0.25, 3))
        .expect("handled")
        .expect("ok");
    assert_eq!(ours.positions.len(), 14);
    assert_eq!(ours.face_sizes.len(), 9);
    assert!((volume(&ours) - 7.968_75).abs() < 1e-5, "{}", volume(&ours));
    assert!(closed_and_oriented(&ours));
}

#[test]
fn every_edge_of_a_cube_is_closed_and_close_to_the_rounded_box_volume() {
    compare(&ALL_EDGES, &opts(0.2, 3), 0.03);
    compare(&ALL_EDGES, &opts(0.2, 1), 0.05);
    compare(&ALL_EDGES, &opts(0.15, 2), 0.04);
    compare(&ALL_EDGES, &opts(0.1, 4), 0.02);
    compare(&ALL_EDGES, &opts(0.1, 5), 0.02);
    // The counts for three segments: 8 corners of 12 vertices, 6 faces, 36 strip quads and 8 corner patches of 7 faces.
    let three = native(&cube(), &ALL_EDGES, &opts(0.2, 3)).expect("handled").expect("ok");
    assert_eq!(three.positions.len(), 96);
    assert_eq!(three.face_sizes.len(), 98);
}

#[test]
fn two_edges_at_a_corner_and_a_ring_of_four_are_closed() {
    // Two edges that meet at vertex 4, and the four edges around the top face.
    for edges in [
        vec![[4, 5], [4, 7]],
        vec![[4, 5], [5, 6], [6, 7], [7, 4]],
        vec![[4, 5], [5, 6]],
    ] {
        for segments in [1, 2, 3, 4] {
            let ours = native(&cube(), &edges, &opts(0.2, segments))
                .expect("handled")
                .expect("ok");
            assert!(closed_and_oriented(&ours), "{edges:?} x {segments}");
            assert_eq!(euler(&ours), 2, "{edges:?} x {segments}");
            let v = volume(&ours);
            assert!(v < 8.0 && v > 7.7, "{edges:?} x {segments}: volume {v}");
        }
    }
}

#[test]
fn the_bounding_box_never_moves_and_origins_point_at_input_vertices() {
    let ours = native(&cube(), &ALL_EDGES, &opts(0.3, 4))
        .expect("handled")
        .expect("ok");
    for axis in 0..3 {
        let high = ours
            .positions
            .iter()
            .map(|p| p[axis])
            .fold(f32::MIN, f32::max);
        let low = ours
            .positions
            .iter()
            .map(|p| p[axis])
            .fold(f32::MAX, f32::min);
        assert!(
            (high - 1.0).abs() < 1e-5 && (low + 1.0).abs() < 1e-5,
            "axis {axis}: {low} {high}"
        );
    }
    assert!(
        ours.vertex_origin
            .iter()
            .all(|&v| v == BeveledMesh::NO_ORIGIN || v < 8)
    );
    for face in 0..6_u32 {
        assert!(ours.face_parent.contains(&face));
    }
}

#[test]
fn offset_kinds_scale_the_cut_as_documented() {
    // On a right angle the width of the bevel face is offset * sqrt(2), so Width 0.2*sqrt(2) equals Offset 0.2.
    let by_offset = native(&cube(), &[[4, 5]], &opts(0.2, 2))
        .expect("handled")
        .expect("ok");
    let mut width = opts(0.2 * std::f32::consts::SQRT_2, 2);
    width.offset_type = BevelOffset::Width;
    let by_width = native(&cube(), &[[4, 5]], &width)
        .expect("handled")
        .expect("ok");
    assert!((volume(&by_offset) - volume(&by_width)).abs() < 1e-5);
    // Depth: perpendicular depth = offset * cos(45 degrees).
    let mut depth = opts(0.2 * std::f32::consts::FRAC_1_SQRT_2, 2);
    depth.offset_type = BevelOffset::Depth;
    let by_depth = native(&cube(), &[[4, 5]], &depth)
        .expect("handled")
        .expect("ok");
    assert!((volume(&by_offset) - volume(&by_depth)).abs() < 1e-5);
}

#[test]
fn an_oversized_width_is_clamped_and_still_closed() {
    let ours = native(&cube(), &ALL_EDGES, &opts(50.0, 3))
        .expect("handled")
        .expect("ok");
    assert!(closed_and_oriented(&ours));
    assert_eq!(euler(&ours), 2);
    assert!(volume(&ours) > 0.0);
}

#[test]
fn the_result_is_deterministic() {
    let a = native(&cube(), &ALL_EDGES, &opts(0.2, 3))
        .expect("handled")
        .expect("ok");
    let b = native(&cube(), &ALL_EDGES, &opts(0.2, 3))
        .expect("handled")
        .expect("ok");
    assert_eq!(a, b);
}

/// A prism over a counter-clockwise polygon, `height` tall: bottom, top and one quad per side.
fn prism(outline: &[[f32; 2]], height: f32) -> (Vec<[f32; 3]>, Vec<u32>, Vec<u32>) {
    let n = outline.len() as u32;
    let mut positions: Vec<[f32; 3]> = outline.iter().map(|p| [p[0], p[1], 0.0]).collect();
    positions.extend(outline.iter().map(|p| [p[0], p[1], height]));
    let mut sizes = vec![n, n];
    let mut corners: Vec<u32> = (0..n).rev().collect();
    corners.extend(n..2 * n);
    for i in 0..n {
        let j = (i + 1) % n;
        sizes.push(4);
        corners.extend([i, j, n + j, n + i]);
    }
    (positions, sizes, corners)
}

fn all_edges(sizes: &[u32], corners: &[u32]) -> Vec<[u32; 2]> {
    let mut seen = std::collections::BTreeSet::new();
    let mut at = 0;
    for &size in sizes {
        let face = &corners[at..at + size as usize];
        for i in 0..face.len() {
            let (a, b) = (face[i], face[(i + 1) % face.len()]);
            seen.insert((a.min(b), a.max(b)));
        }
        at += size as usize;
    }
    seen.into_iter().map(|(a, b)| [a, b]).collect()
}

fn check_prism(outline: &[[f32; 2]], height: f32, width: f32, segments: u32, tolerance: f64) {
    let (positions, sizes, corners) = prism(outline, height);
    let mesh = PolygonMesh {
        positions: &positions,
        face_sizes: &sizes,
        corners: &corners,
    };
    let edges = all_edges(&sizes, &corners);
    let options = opts(width, segments);
    let ours = native(&mesh, &edges, &options)
        .unwrap_or_else(|| panic!("native declined {outline:?}"))
        .expect("native result");
    assert!(
        closed_and_oriented(&ours),
        "{outline:?} not closed and oriented"
    );
    assert_eq!(euler(&ours), 2, "{outline:?}");
    let a = volume(&ours);
    // Rounding a small width takes off a sliver: at most `tolerance * 20` of the prism (a loose bound, the exact
    // amount depends on the plan), and some material, which the check below confirms.
    let sharp = volume(&BeveledMesh {
        positions: positions.clone(),
        face_sizes: sizes.clone(),
        corners: corners.clone(),
        vertex_origin: vec![],
        face_parent: vec![],
        face_role: vec![],
    });
    assert!(a > sharp * (1.0 - tolerance * 20.0), "{outline:?}: volume {a} of {sharp}");
    assert!(
        a < volume(&BeveledMesh {
            positions: positions.clone(),
            face_sizes: sizes.clone(),
            corners: corners.clone(),
            vertex_origin: vec![],
            face_parent: vec![],
            face_role: vec![],
        })
    );
}

#[test]
fn a_hexagonal_prism_with_every_edge_beveled() {
    let hex: Vec<[f32; 2]> = (0..6)
        .map(|i| {
            let a = i as f32 * std::f32::consts::FRAC_PI_3;
            [a.cos(), a.sin()]
        })
        .collect();
    check_prism(&hex, 1.0, 0.1, 3, 0.01);
    check_prism(&hex, 1.0, 0.1, 1, 0.01);
}

#[test]
fn a_box_house_with_a_wide_plan_and_every_edge_beveled() {
    let plan = [[0.0, 0.0], [8.0, 0.0], [8.0, 5.0], [0.0, 5.0]];
    check_prism(&plan, 3.0, 0.05, 2, 0.005);
}

#[test]
fn an_l_shaped_plan_with_a_concave_corner() {
    let plan = [
        [0.0, 0.0],
        [4.0, 0.0],
        [4.0, 2.0],
        [2.0, 2.0],
        [2.0, 4.0],
        [0.0, 4.0],
    ];
    check_prism(&plan, 2.0, 0.1, 3, 0.01);
}

fn area(mesh: &BeveledMesh) -> f64 {
    let mut total = 0.0;
    let mut at = 0;
    for &size in &mesh.face_sizes {
        let c = &mesh.corners[at..at + size as usize];
        let p = |i: u32| mesh.positions[i as usize].map(f64::from);
        let mut n = [0.0; 3];
        for i in 0..c.len() {
            let (a, b) = (p(c[i]), p(c[(i + 1) % c.len()]));
            n[0] += (a[1] - b[1]) * (a[2] + b[2]);
            n[1] += (a[2] - b[2]) * (a[0] + b[0]);
            n[2] += (a[0] - b[0]) * (a[1] + b[1]);
        }
        total += (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt() / 2.0;
        at += size as usize;
    }
    total
}

#[test]
fn one_beveled_edge_at_a_vertex_with_four_faces() {
    // The cube with its top face split along the diagonal 4-6: vertex 4 has four faces, and only edge 5-6 is beveled
    // (an edge that does not touch the diagonal's far end).
    let mut sizes = vec![4; 5];
    let mut corners: Vec<u32> = vec![0, 3, 2, 1, 0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6, 3, 0, 4, 7];
    sizes.extend([3, 3]);
    corners.extend([4, 5, 6, 4, 6, 7]);
    let mesh = PolygonMesh { positions: &CUBE_POSITIONS, face_sizes: &sizes, corners: &corners };
    for edge in [[1, 5], [0, 4]] {
        for segments in [1, 2, 3] {
            let options = opts(0.2, segments);
            let ours = native(&mesh, &[edge], &options)
                .unwrap_or_else(|| panic!("native declined {edge:?} x {segments}"))
                .expect("native result");
            assert!(closed_and_oriented(&ours), "{edge:?} x {segments}");
            assert_eq!(euler(&ours), 2, "{edge:?} x {segments}");
            let v = volume(&ours);
            assert!(v < 8.0 && v > 7.9, "{edge:?} x {segments}: volume {v}");
        }
    }
}

#[test]
fn a_hinge_edge_on_an_open_folded_sheet() {
    let positions = [
        [0.0, 0.0, 0.0],
        [2.0, 0.0, 0.0],
        [2.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [2.0, 0.0, 1.0],
    ];
    let mesh = PolygonMesh { positions: &positions, face_sizes: &[4, 4], corners: &[0, 1, 2, 3, 1, 0, 4, 5] };
    for segments in [1, 2, 4] {
        let options = opts(0.2, segments);
        let ours = native(&mesh, &[[0, 1]], &options).expect("handled").expect("ok");
        // Every edge has one or two faces, never more, and no edge is walked twice the same way.
        let mut directed: BTreeMap<(u32, u32), u32> = BTreeMap::new();
        let mut at = 0;
        for &size in &ours.face_sizes {
            let f = &ours.corners[at..at + size as usize];
            for i in 0..f.len() {
                *directed.entry((f[i], f[(i + 1) % f.len()])).or_default() += 1;
            }
            at += size as usize;
        }
        assert!(directed.values().all(|&n| n == 1), "an edge is walked twice the same way ({segments})");
        let boundary = directed.keys().filter(|&&(a, b)| !directed.contains_key(&(b, a))).count();
        assert!(boundary >= 6, "{boundary}");
        // Two original faces, `segments` strip quads between the cuts, and no caps (the sheet stays open).
        assert_eq!(ours.face_sizes.len(), 2 + segments as usize);
        let total = area(&ours);
        assert!(total > 3.6 && total < 4.2, "{segments}: area {total}");
    }
}
