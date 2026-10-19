//! UV unwrapping (Blender's parametrizer and island packer) through the safe API.

use ve_kernel::{
    KernelError, PolygonMesh, UnwrapOptions, UvMethod, UvPackOptions, UvPackShape, pack_uvs,
    unwrap_uvs,
};

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
const CORNERS: [u32; 24] = [
    0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6, 3, 0, 4, 7,
];
const CROSS_SEAMS: [[u32; 2]; 7] = [[0, 4], [1, 5], [2, 6], [3, 7], [4, 5], [5, 6], [4, 7]];
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
        positions: &POSITIONS,
        face_sizes: &[4; 6],
        corners: &CORNERS,
    }
}

fn options(method: UvMethod, pack: bool) -> UnwrapOptions {
    UnwrapOptions {
        method,
        pack,
        rotate: false,
        margin: 0.02,
    }
}

/// Area of face `face` of the cube in UV space.
fn face_area(uvs: &[[f32; 2]], face: usize) -> f64 {
    let quad = &uvs[4 * face..4 * face + 4];
    let mut area = 0.0_f64;
    for k in 0..4 {
        let (a, b) = (quad[k], quad[(k + 1) % 4]);
        area += f64::from(a[0]) * f64::from(b[1]) - f64::from(b[0]) * f64::from(a[1]);
    }
    area / 2.0
}

#[test]
fn cross_net_unfolds_without_stretch() {
    for method in [UvMethod::Conformal, UvMethod::AngleBased] {
        let layout = unwrap_uvs(&cube(), &CROSS_SEAMS, &options(method, false)).unwrap();
        assert_eq!(layout.failed_islands, 0);
        assert_eq!(layout.uvs.len(), 24);
        for face in 0..6 {
            let area = face_area(&layout.uvs, face).abs();
            assert!((area - 4.0).abs() < 1e-3, "{method:?} face {face}: {area}");
        }
    }
}

#[test]
fn packed_islands_stay_in_the_unit_square_and_apart() {
    let layout = unwrap_uvs(&cube(), &ALL_EDGES, &options(UvMethod::Conformal, true)).unwrap();
    assert_eq!(layout.failed_islands, 0);
    let boxes: Vec<[f32; 4]> = (0..6)
        .map(|face| {
            let quad = &layout.uvs[4 * face..4 * face + 4];
            let low = |axis: usize| quad.iter().map(|p| p[axis]).fold(f32::MAX, f32::min);
            let high = |axis: usize| quad.iter().map(|p| p[axis]).fold(f32::MIN, f32::max);
            [low(0), low(1), high(0), high(1)]
        })
        .collect();
    for b in &boxes {
        assert!(b.iter().all(|v| (-1e-6..=1.0 + 1e-6).contains(v)), "{b:?}");
    }
    for (i, a) in boxes.iter().enumerate() {
        for b in &boxes[i + 1..] {
            let apart = a[2] <= b[0] || b[2] <= a[0] || a[3] <= b[1] || b[3] <= a[1];
            assert!(apart, "{a:?} overlaps {b:?}");
        }
    }
}

#[test]
fn bad_input_is_refused() {
    let opts = options(UvMethod::Conformal, false);
    // A space diagonal is not an edge.
    assert_eq!(
        unwrap_uvs(&cube(), &[[0, 6]], &opts).unwrap_err(),
        KernelError::InvalidArgument("rejected by the kernel")
    );
    assert_eq!(
        unwrap_uvs(&cube(), &[[0, 99]], &opts).unwrap_err(),
        KernelError::OutOfBounds
    );
    let too_wide = UnwrapOptions {
        margin: 0.9,
        ..opts
    };
    assert!(unwrap_uvs(&cube(), &CROSS_SEAMS, &too_wide).is_err());
    let nan = UnwrapOptions {
        margin: f32::NAN,
        ..opts
    };
    assert!(unwrap_uvs(&cube(), &CROSS_SEAMS, &nan).is_err());
}

#[test]
fn existing_uv_islands_pack_deterministically_inside_unit_square() {
    let mesh = cube();
    let unwrapped = unwrap_uvs(&mesh, &CROSS_SEAMS, &options(UvMethod::Conformal, false)).unwrap();
    let settings = UvPackOptions {
        rotate: true,
        keep_density: false,
        shape: UvPackShape::Aabb,
        margin: 0.02,
    };
    let first = pack_uvs(&mesh, &unwrapped.uvs, &settings).unwrap();
    let second = pack_uvs(&mesh, &unwrapped.uvs, &settings).unwrap();
    assert_eq!(first, second);
    assert!(first.iter().flatten().all(|uv| (0.0..=1.0).contains(uv)));
}
