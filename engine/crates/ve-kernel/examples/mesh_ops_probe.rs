//! One line per Blender mesh operator on a cube (counts and a hash of every output bit), for comparing
//! native and wasm32-wasip1 output: run natively with `cargo run -p ve-kernel --example mesh_ops_probe`
//! and under Node WASI with `tools/run-wasi.mjs`, and the two outputs must be identical.
#![expect(
    clippy::print_stdout,
    clippy::unwrap_used,
    reason = "a probe that prints its findings"
)]

use ve_kernel::{MeshOp, MeshSelection, PokeCenter, PolygonMesh, edit_mesh};

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

fn report(name: &str, mesh: &PolygonMesh<'_>, selection: &MeshSelection<'_>, op: &MeshOp) {
    let result = edit_mesh(mesh, selection, op).unwrap();
    let hash = result
        .positions
        .iter()
        .flatten()
        .map(|value| u64::from(value.to_bits()))
        .chain(result.corners.iter().map(|&c| u64::from(c)))
        .chain(result.face_parent.iter().map(|&p| u64::from(p)))
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, value| {
            (hash ^ value).wrapping_mul(0x0100_0000_01b3)
        });
    println!(
        "{name} vertices={} faces={} hash={hash:016x}",
        result.positions.len(),
        result.face_sizes.len()
    );
}

fn main() {
    let cube = PolygonMesh {
        positions: &POSITIONS,
        face_sizes: &[4; 6],
        corners: &CORNERS,
    };
    let top = MeshSelection {
        faces: &[1],
        edges: &[],
        vertices: &[],
    };
    let whole = MeshSelection::default();
    report(
        "inset",
        &cube,
        &top,
        &MeshOp::Inset {
            thickness: 0.2,
            depth: 0.1,
            even: true,
            relative: false,
            boundary: true,
            outset: false,
        },
    );
    report(
        "extrude",
        &cube,
        &top,
        &MeshOp::Extrude {
            offset: [0.0, 0.0, 0.5],
        },
    );
    report(
        "poke",
        &cube,
        &top,
        &MeshOp::Poke {
            offset: 0.5,
            center: PokeCenter::Weighted,
            relative: false,
        },
    );
    report(
        "solidify",
        &cube,
        &whole,
        &MeshOp::Solidify { thickness: 0.1 },
    );
    report(
        "subdivide",
        &cube,
        &MeshSelection {
            faces: &[],
            edges: &[[4, 5], [5, 6]],
            vertices: &[],
        },
        &MeshOp::Subdivide {
            cuts: 2,
            smooth: 0.5,
            grid_fill: true,
        },
    );
    report(
        "wireframe",
        &cube,
        &whole,
        &MeshOp::Wireframe {
            thickness: 0.1,
            offset: 0.0,
            even: true,
            relative: false,
            boundary: true,
            replace: true,
        },
    );
    let tri_corners: Vec<u32> = CORNERS
        .chunks(4)
        .flat_map(|q| [q[0], q[1], q[2], q[0], q[2], q[3]])
        .collect();
    let tri = PolygonMesh {
        positions: &POSITIONS,
        face_sizes: &[3; 12],
        corners: &tri_corners,
    };
    report(
        "dissolve",
        &tri,
        &whole,
        &MeshOp::Dissolve {
            angle: 0.3,
            boundaries: false,
        },
    );
    // A bumpy 16 x 16 grid: split into triangles for decimation, left as quads for unsubdivision.
    let n = 16_u32;
    let mut grid = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            let (x, y) = (f32::from(i as u8) / 16.0, f32::from(j as u8) / 16.0);
            grid.push([x, y, 0.05 * (6.0 * x).sin() * (5.0 * y).cos()]);
        }
    }
    let (mut quads, mut tris) = (Vec::new(), Vec::new());
    for j in 0..n {
        for i in 0..n {
            let a = j * (n + 1) + i;
            let (b, c, d) = (a + 1, a + n + 2, a + n + 1);
            quads.extend_from_slice(&[a, b, c, d]);
            tris.extend_from_slice(&[a, b, c, a, c, d]);
        }
    }
    let triangles = PolygonMesh {
        positions: &grid,
        face_sizes: &vec![3; tris.len() / 3],
        corners: &tris,
    };
    report(
        "decimate",
        &triangles,
        &whole,
        &MeshOp::Decimate {
            ratio: 0.3,
            symmetry: None,
            triangulate: true,
        },
    );
    let squares = PolygonMesh {
        positions: &grid,
        face_sizes: &vec![4; quads.len() / 4],
        corners: &quads,
    };
    report(
        "unsubdivide",
        &squares,
        &whole,
        &MeshOp::Unsubdivide { iterations: 2 },
    );
    report(
        "symmetrize",
        &cube,
        &whole,
        &MeshOp::Symmetrize {
            axis: 2,
            keep_positive: false,
            distance: 1e-4,
        },
    );
}
