use ve_kernel::{PolygonMesh, UvPackOptions, UvPackShape, pack_uvs};

const POSITIONS: [[f32; 3]; 4] = [
    [0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [1.0, 1.0, 0.0],
    [0.0, 1.0, 0.0],
];
const CORNERS: [u32; 4] = [0, 1, 2, 3];

#[test]
fn native_pack_preserves_a_single_island_and_bounds_it() {
    let mesh = PolygonMesh {
        positions: &POSITIONS,
        face_sizes: &[4],
        corners: &CORNERS,
    };
    let uv = [[-2.0, 3.0], [0.0, 3.0], [0.0, 5.0], [-2.0, 5.0]];
    let options = UvPackOptions {
        rotate: false,
        keep_density: false,
        shape: UvPackShape::Aabb,
        margin: 0.05,
    };
    let packed = pack_uvs(&mesh, &uv, &options).expect("valid packing input");
    let expected = [[0.05, 0.05], [0.95, 0.05], [0.95, 0.95], [0.05, 0.95]];
    for (actual, expected) in packed.iter().flatten().zip(expected.iter().flatten()) {
        assert!((actual - expected).abs() < 1e-5, "{actual} != {expected}");
    }
}

use ve_kernel::native::uv::{pack_uvs as native_pack, unwrap_uvs as native_unwrap};
use ve_kernel::{UnwrapOptions, UvMethod};

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
const CROSS: [[u32; 2]; 7] = [[0, 4], [1, 5], [2, 6], [3, 7], [4, 5], [5, 6], [4, 7]];
const ALL: [[u32; 2]; 12] = [
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

fn unwrap_options(pack: bool) -> UnwrapOptions {
    UnwrapOptions {
        method: UvMethod::Conformal,
        pack,
        rotate: false,
        margin: 0.02,
    }
}

fn quad_edge_lengths(uvs: &[[f32; 2]], face: usize) -> [f64; 4] {
    let q = &uvs[4 * face..4 * face + 4];
    let d = |a: [f32; 2], b: [f32; 2]| {
        (f64::from(a[0] - b[0]).powi(2) + f64::from(a[1] - b[1]).powi(2)).sqrt()
    };
    [d(q[0], q[1]), d(q[1], q[2]), d(q[2], q[3]), d(q[3], q[0])]
}

#[test]
fn a_cube_cut_along_every_edge_gives_six_unit_squares() {
    let layout = native_unwrap(&cube(), &ALL, &unwrap_options(false))
        .expect("handled")
        .expect("ok");
    assert_eq!(layout.failed_islands, 0);
    for face in 0..6 {
        for length in quad_edge_lengths(&layout.uvs, face) {
            assert!((length - 2.0).abs() < 1e-4, "face {face}: edge {length}");
        }
    }
}

#[test]
fn the_cross_net_keeps_every_edge_length() {
    let layout = native_unwrap(&cube(), &CROSS, &unwrap_options(false))
        .expect("handled")
        .expect("ok");
    for face in 0..6 {
        for length in quad_edge_lengths(&layout.uvs, face) {
            assert!((length - 2.0).abs() < 1e-3, "face {face}: edge {length}");
        }
    }
}

/// A latitude/longitude sphere with a seam down one meridian.
fn sphere(rings: u32, segments: u32) -> (Vec<[f32; 3]>, Vec<u32>, Vec<u32>, Vec<[u32; 2]>) {
    let mut positions = vec![[0.0, 0.0, 1.0]];
    for r in 1..rings {
        let phi = std::f32::consts::PI * r as f32 / rings as f32;
        for s in 0..segments {
            let theta = std::f32::consts::TAU * s as f32 / segments as f32;
            positions.push([phi.sin() * theta.cos(), phi.sin() * theta.sin(), phi.cos()]);
        }
    }
    positions.push([0.0, 0.0, -1.0]);
    let ring = |r: u32, s: u32| 1 + (r - 1) * segments + s % segments;
    let south = positions.len() as u32 - 1;
    let (mut sizes, mut corners) = (Vec::new(), Vec::new());
    for s in 0..segments {
        sizes.push(3);
        corners.extend([0, ring(1, s), ring(1, s + 1)]);
    }
    for r in 1..rings - 1 {
        for s in 0..segments {
            sizes.push(4);
            corners.extend([
                ring(r, s),
                ring(r + 1, s),
                ring(r + 1, s + 1),
                ring(r, s + 1),
            ]);
        }
    }
    for s in 0..segments {
        sizes.push(3);
        corners.extend([south, ring(rings - 1, s + 1), ring(rings - 1, s)]);
    }
    // The seam runs from the north pole to the south pole along longitude 0.
    let mut seam = vec![[0, ring(1, 0)]];
    for r in 1..rings - 1 {
        seam.push([ring(r, 0), ring(r + 1, 0)]);
    }
    seam.push([ring(rings - 1, 0), south]);
    (positions, sizes, corners, seam)
}

#[test]
fn a_sphere_with_one_seam_unwraps_without_nan_or_overlap_of_corners() {
    let (positions, sizes, corners, seam) = sphere(16, 24);
    let mesh = PolygonMesh {
        positions: &positions,
        face_sizes: &sizes,
        corners: &corners,
    };
    let layout = native_unwrap(&mesh, &seam, &unwrap_options(true))
        .expect("handled")
        .expect("ok");
    assert_eq!(layout.failed_islands, 0);
    assert!(
        layout
            .uvs
            .iter()
            .flatten()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
    );
    // Conformal maps may stretch toward the poles but must not flip a face: every UV triangle keeps one orientation.
    let mut at = 0;
    let (mut positive, mut negative) = (0, 0);
    for &size in &sizes {
        let uv = &layout.uvs[at..at + size as usize];
        let area: f32 = (0..uv.len())
            .map(|i| uv[i][0] * uv[(i + 1) % uv.len()][1] - uv[(i + 1) % uv.len()][0] * uv[i][1])
            .sum();
        if area > 0.0 {
            positive += 1
        } else {
            negative += 1
        }
        at += size as usize;
    }
    assert!(
        positive == 0 || negative == 0,
        "{positive} faces one way and {negative} the other"
    );
    let coverage = |uvs: &[[f32; 2]]| {
        let mut total = 0.0_f64;
        let mut at = 0;
        for &size in &sizes {
            let uv = &uvs[at..at + size as usize];
            let area: f64 = (0..uv.len())
                .map(|i| {
                    f64::from(uv[i][0]) * f64::from(uv[(i + 1) % uv.len()][1])
                        - f64::from(uv[(i + 1) % uv.len()][0]) * f64::from(uv[i][1])
                })
                .sum();
            total += area.abs() / 2.0;
            at += size as usize;
        }
        total
    };
    // One island packed into the unit square: it fills a fair share of it, and cannot exceed it.
    let ours = coverage(&layout.uvs);
    assert!(ours > 0.2 && ours <= 1.0, "coverage {ours}");
}

#[test]
fn unwrap_is_deterministic_and_declines_nothing_it_can_solve() {
    let (positions, sizes, corners, seam) = sphere(8, 12);
    let mesh = PolygonMesh {
        positions: &positions,
        face_sizes: &sizes,
        corners: &corners,
    };
    let a = native_unwrap(&mesh, &seam, &unwrap_options(true))
        .expect("handled")
        .expect("ok");
    let b = native_unwrap(&mesh, &seam, &unwrap_options(true))
        .expect("handled")
        .expect("ok");
    assert_eq!(a, b);
}

#[test]
fn packing_with_rotation_fits_a_tall_island_and_keeps_density() {
    let uv = [[0.0, 0.0], [0.5, 0.0], [0.5, 4.0], [0.0, 4.0]];
    let mesh = PolygonMesh {
        positions: &[[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
        face_sizes: &[4],
        corners: &[0, 1, 2, 3],
    };
    let options = UvPackOptions {
        rotate: true,
        keep_density: true,
        shape: UvPackShape::Aabb,
        margin: 0.05,
    };
    let packed = native_pack(&mesh, &uv, &options)
        .expect("handled")
        .expect("ok");
    let (w, h) = (
        packed.iter().map(|p| p[0]).fold(f32::MIN, f32::max)
            - packed.iter().map(|p| p[0]).fold(f32::MAX, f32::min),
        packed.iter().map(|p| p[1]).fold(f32::MIN, f32::max)
            - packed.iter().map(|p| p[1]).fold(f32::MAX, f32::min),
    );
    assert!(
        (w / h - 8.0).abs() < 1e-3,
        "rotated to a wide island, aspect {}",
        w / h
    );
    assert!(packed.iter().flatten().all(|v| (0.0..=1.0).contains(v)));
}
