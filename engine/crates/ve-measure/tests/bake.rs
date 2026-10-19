//! Baking a normal map and ambient occlusion from a high-poly mesh onto a low-poly mesh's UV layout, measured on
//! shapes with exact answers: a flat quad, a tilted plane, a mirrored layout, an overhang.
use ve_measure::{LowMesh, MeasureError, Mesh, bake_ao, bake_normals};

const QUAD: [[f32; 3]; 4] = [
    [0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [1.0, 1.0, 0.0],
    [0.0, 1.0, 0.0],
];
const TRIS: [u32; 6] = [0, 1, 2, 0, 2, 3];

/// One UV per triangle corner, from the corner's position by `map`.
fn corner_uvs(map: impl Fn([f32; 3]) -> [f32; 2]) -> Vec<[f32; 2]> {
    TRIS.iter().map(|&i| map(QUAD[i as usize])).collect()
}

fn xy(p: [f32; 3]) -> [f32; 2] {
    [p[0], p[1]]
}

/// The quad with each vertex lifted by `lift(position)`.
fn lifted(lift: impl Fn([f32; 3]) -> f32) -> Vec<[f32; 3]> {
    QUAD.iter().map(|p| [p[0], p[1], p[2] + lift(*p)]).collect()
}

fn pixel(bake: &ve_measure::Bake, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * bake.size + x) * 4) as usize;
    [
        bake.rgba[at],
        bake.rgba[at + 1],
        bake.rgba[at + 2],
        bake.rgba[at + 3],
    ]
}

fn near(actual: u8, expected: i32, tolerance: i32, what: &str) {
    assert!(
        (i32::from(actual) - expected).abs() <= tolerance,
        "{what}: {actual}, expected {expected} +/- {tolerance}"
    );
}

#[test]
fn a_flat_surface_bakes_the_flat_normal() {
    let uvs = corner_uvs(xy);
    let low = LowMesh::new(&QUAD, &TRIS, &uvs).unwrap();
    let high = Mesh::new(&QUAD, &TRIS);
    let bake = bake_normals(&low, &high, 32, 0.1, 0.5).unwrap();
    assert!(bake.covered >= 32 * 32 - 2 * 32, "covered {}", bake.covered);
    for y in 0..32 {
        for x in 0..32 {
            let p = pixel(&bake, x, y);
            if p[3] == 255 {
                near(p[0], 128, 1, "red");
                near(p[1], 128, 1, "green");
                near(p[2], 255, 1, "blue");
            }
        }
    }
}

#[test]
fn a_tilted_high_surface_bakes_its_tangent_space_normal() {
    // z = 0.5 x: normal (-0.4472, 0, 0.8944), encoded about (70, 128, 242). The cage starts above the highest point.
    let rising = lifted(|p| 0.5 * p[0]);
    let uvs = corner_uvs(xy);
    let low = LowMesh::new(&QUAD, &TRIS, &uvs).unwrap();
    let bake = bake_normals(&low, &Mesh::new(&rising, &TRIS), 32, 0.6, 1.0).unwrap();
    let mut checked = 0;
    for y in 0..32 {
        for x in 0..32 {
            let p = pixel(&bake, x, y);
            if p[3] == 255 {
                near(p[0], 70, 2, "red");
                near(p[1], 128, 2, "green");
                near(p[2], 242, 2, "blue");
                checked += 1;
            }
        }
    }
    assert!(
        checked >= 32 * 32 - 64,
        "only {checked} texels were checked"
    );
}

#[test]
fn a_mirrored_layout_mirrors_the_red_channel() {
    // u = 1 - x: the tangent points toward -x, so the same normal reads +0.4472 in red (about 186).
    let rising = lifted(|p| 0.5 * p[0]);
    let uvs = corner_uvs(|p| [1.0 - p[0], p[1]]);
    let low = LowMesh::new(&QUAD, &TRIS, &uvs).unwrap();
    let bake = bake_normals(&low, &Mesh::new(&rising, &TRIS), 32, 0.6, 1.0).unwrap();
    let p = pixel(&bake, 16, 16);
    near(p[0], 186, 2, "red");
    near(p[1], 128, 2, "green");
    near(p[2], 242, 2, "blue");
}

#[test]
fn max_distance_decides_what_is_reached() {
    // The tilted plane sits one metre below: out of reach for 0.5, within reach for 2.0.
    let deep = lifted(|p| 0.5 * p[0] - 1.0);
    let uvs = corner_uvs(xy);
    let low = LowMesh::new(&QUAD, &TRIS, &uvs).unwrap();
    let missed = bake_normals(&low, &Mesh::new(&deep, &TRIS), 32, 0.1, 0.5).unwrap();
    let hit = bake_normals(&low, &Mesh::new(&deep, &TRIS), 32, 0.1, 2.0).unwrap();
    let (m, h) = (pixel(&missed, 16, 16), pixel(&hit, 16, 16));
    near(m[0], 128, 1, "missed red");
    near(m[2], 255, 1, "missed blue");
    near(h[0], 70, 2, "hit red");
    near(h[2], 242, 2, "hit blue");
}

#[test]
fn padding_copies_outward_four_texels_and_no_further() {
    let uvs = corner_uvs(|p| [p[0] * 0.5, p[1]]);
    let low = LowMesh::new(&QUAD, &TRIS, &uvs).unwrap();
    let bake = bake_normals(&low, &Mesh::new(&QUAD, &TRIS), 32, 0.1, 0.5).unwrap();
    let half = 0.5 * 32.0 * 32.0;
    assert!(
        (f64::from(bake.covered) - half).abs() / half < 0.03,
        "covered {}",
        bake.covered
    );
    for y in 0..32 {
        assert_eq!(pixel(&bake, 15, y)[3], 255, "last covered column, row {y}");
        assert_eq!(
            pixel(&bake, 19, y)[3],
            255,
            "padded column (4 texels out), row {y}"
        );
        assert_eq!(pixel(&bake, 20, y)[3], 0, "beyond the padding, row {y}");
        assert_eq!(pixel(&bake, 31, y)[3], 0, "far column, row {y}");
    }
}

#[test]
fn an_open_flat_surface_sees_the_whole_sky() {
    let uvs = corner_uvs(xy);
    let low = LowMesh::new(&QUAD, &TRIS, &uvs).unwrap();
    let bake = bake_ao(&low, &Mesh::new(&QUAD, &TRIS), 16, 64, 5.0, 0.001).unwrap();
    for y in 0..16 {
        for x in 0..16 {
            let p = pixel(&bake, x, y);
            if p[3] == 255 {
                near(p[0], 255, 3, "open surface");
            }
        }
    }
}

#[test]
fn an_overhang_shadows_the_middle_more_than_the_edge() {
    // The footprint plus a second quad 0.2 above it: the middle is almost closed in, the edge is half open.
    let mut positions = QUAD.to_vec();
    positions.extend(lifted(|_| 0.2));
    let mut indices = TRIS.to_vec();
    indices.extend(TRIS.iter().map(|i| i + 4));
    let uvs = corner_uvs(xy);
    let low = LowMesh::new(&QUAD, &TRIS, &uvs).unwrap();
    let bake = bake_ao(&low, &Mesh::new(&positions, &indices), 32, 256, 5.0, 0.001).unwrap();
    let (middle, edge) = (pixel(&bake, 16, 16)[0], pixel(&bake, 0, 16)[0]);
    assert!(middle < 38, "middle {middle} should be under 15% open");
    assert!(edge > 102, "edge {edge} should be over 40% open");
    assert!(
        edge > middle,
        "edge {edge} should be more open than the middle {middle}"
    );
}

#[test]
fn baking_is_repeatable_and_resolution_independent() {
    let rising = lifted(|p| 0.5 * p[0]);
    let uvs = corner_uvs(xy);
    let low = LowMesh::new(&QUAD, &TRIS, &uvs).unwrap();
    let high = Mesh::new(&rising, &TRIS);
    let first = bake_normals(&low, &high, 32, 0.6, 1.0).unwrap();
    assert_eq!(first, bake_normals(&low, &high, 32, 0.6, 1.0).unwrap());
    let ao = bake_ao(&low, &high, 16, 32, 5.0, 0.001).unwrap();
    assert_eq!(ao, bake_ao(&low, &high, 16, 32, 5.0, 0.001).unwrap());
    let big = bake_normals(&low, &high, 64, 0.6, 1.0).unwrap();
    for channel in 0..3 {
        near(
            pixel(&big, 32, 32)[channel],
            i32::from(pixel(&first, 16, 16)[channel]),
            1,
            "centre texel across sizes",
        );
    }
}

#[test]
fn bad_input_is_a_typed_error() {
    let uvs = corner_uvs(xy);
    // Wrong lengths, an index out of range, a NaN position, a NaN uv.
    assert!(LowMesh::new(&QUAD, &TRIS[..4], &uvs[..4]).is_err());
    assert!(LowMesh::new(&QUAD, &[0, 1, 9], &uvs[..3]).is_err());
    let nan = [[f32::NAN, 0.0, 0.0], QUAD[1], QUAD[2], QUAD[3]];
    assert!(matches!(
        LowMesh::new(&nan, &TRIS, &uvs),
        Err(MeasureError::NonFinite(0))
    ));
    assert!(matches!(
        LowMesh::new(&QUAD, &TRIS, &uvs[..5]),
        Err(MeasureError::Bake(_))
    ));
    let bad_uv: Vec<[f32; 2]> = uvs.iter().map(|_| [f32::NAN, 0.0]).collect();
    assert!(matches!(
        LowMesh::new(&QUAD, &TRIS, &bad_uv),
        Err(MeasureError::Bake(_))
    ));
    let low = LowMesh::new(&QUAD, &TRIS, &uvs).unwrap();
    let high = Mesh::new(&QUAD, &TRIS);
    assert!(matches!(
        bake_normals(&low, &high, 4, 0.1, 0.5),
        Err(MeasureError::Bake(_))
    ));
    assert!(matches!(
        bake_normals(&low, &high, 4096, 0.1, 0.5),
        Err(MeasureError::Bake(_))
    ));
    assert!(matches!(
        bake_ao(&low, &high, 32, 0, 1.0, 0.001),
        Err(MeasureError::Bake(_))
    ));
    assert!(matches!(
        bake_ao(&low, &high, 32, 1000, 1.0, 0.001),
        Err(MeasureError::Bake(_))
    ));
    assert!(matches!(
        bake_ao(&low, &high, 2048, 256, 1.0, 0.001),
        Err(MeasureError::Bake(_))
    ));
}

/// A unit cube on eight shared vertices (outward winding) with each face in its own tile of a 3 x 2 atlas.
fn cube() -> (Vec<[f32; 3]>, Vec<u32>, Vec<[f32; 2]>) {
    let positions: Vec<[f32; 3]> = (0..8)
        .map(|i| {
            let s = |bit: u32| if i & bit == 0 { -0.5 } else { 0.5 };
            [s(1), s(2), s(4)]
        })
        .collect();
    // Corner order of each face, counter-clockwise seen from outside: -z, +z, -x, +x, -y, +y.
    let faces: [[u32; 4]; 6] = [
        [0, 2, 3, 1],
        [4, 5, 7, 6],
        [0, 4, 6, 2],
        [1, 3, 7, 5],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
    ];
    let (mut indices, mut uvs) = (Vec::new(), Vec::new());
    for (face, quad) in (0_u8..).zip(faces.iter()) {
        let (column, row) = (f32::from(face % 3), f32::from(face / 3));
        let corner_uv = |k: usize| {
            let (u, v) = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)][k];
            [
                (column + 0.02 + 0.96 * u) / 3.0,
                (row + 0.02 + 0.96 * v) / 2.0,
            ]
        };
        for k in [0, 1, 2, 0, 2, 3] {
            indices.push(quad[k]);
            uvs.push(corner_uv(k));
        }
    }
    (positions, indices, uvs)
}

#[test]
fn a_hard_edged_cube_bakes_flat_against_itself() {
    // Smoothing normals across the 90 degree edges would tilt every face toward its corners; with a crease angle the
    // faces stay flat, so a cube baked against itself is the flat normal everywhere.
    let (positions, indices, uvs) = cube();
    let low = LowMesh::new(&positions, &indices, &uvs).unwrap();
    let bake = bake_normals(&low, &Mesh::new(&positions, &indices), 96, 0.1, 0.5).unwrap();
    let mut checked = 0;
    for y in 0..96 {
        for x in 0..96 {
            let p = pixel(&bake, x, y);
            if p[3] == 255 {
                near(p[0], 128, 1, "red");
                near(p[1], 128, 1, "green");
                near(p[2], 255, 1, "blue");
                checked += 1;
            }
        }
    }
    assert!(checked > 96 * 96 / 2, "only {checked} texels covered");
}

#[test]
fn a_hard_edged_cube_is_fully_open_to_the_sky() {
    let (positions, indices, uvs) = cube();
    let low = LowMesh::new(&positions, &indices, &uvs).unwrap();
    let bake = bake_ao(&low, &Mesh::new(&positions, &indices), 48, 64, 5.0, 0.001).unwrap();
    for y in 0..48 {
        for x in 0..48 {
            let p = pixel(&bake, x, y);
            if p[3] == 255 {
                near(p[0], 255, 2, "a convex shape occludes nothing of itself");
            }
        }
    }
}
