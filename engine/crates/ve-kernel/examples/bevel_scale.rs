//! Time Blender's bevel on large meshes: a cube whose six faces are `n` by `n` grids (6 n^2 quads), with
//! the twelve cube edges beveled. Run natively and as wasm32-wasip1 to compare:
//! `cargo run --release -p ve-kernel --example bevel_scale -- 100 200`.
#![expect(
    clippy::print_stdout,
    reason = "reports timings through stdout for the person running it"
)]

use std::collections::HashMap;
use std::time::Instant;

use ve_kernel::{BevelOffset, BevelOptions, PolygonMesh, bevel_edges};

/// A cube surface with welded vertices on the lattice 0..=n, faces wound outward.
fn grid_cube(n: u32) -> (Vec<[f32; 3]>, Vec<u32>, Vec<u32>, Vec<[u32; 2]>) {
    let mut ids: HashMap<[u32; 3], u32> = HashMap::new();
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut vertex = |p: [u32; 3]| -> u32 {
        *ids.entry(p).or_insert_with(|| {
            positions.push(p.map(|c| {
                f32::from(u16::try_from(c).unwrap_or(0)) / f32::from(u16::try_from(n).unwrap_or(1))
            }));
            u32::try_from(positions.len() - 1).unwrap_or(0)
        })
    };
    let (mut sizes, mut corners) = (Vec::new(), Vec::new());
    for axis in 0..3_usize {
        for side in [0, n] {
            let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
            for i in 0..n {
                for j in 0..n {
                    let at = |a: u32, b: u32| {
                        let mut p = [0_u32; 3];
                        p[axis] = side;
                        p[u] = a;
                        p[v] = b;
                        p
                    };
                    let mut quad = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
                    if side == 0 {
                        quad.reverse();
                    }
                    sizes.push(4);
                    corners.extend(quad.map(&mut vertex));
                }
            }
        }
    }
    // The twelve cube edges, as unit lattice steps.
    let mut edges = Vec::new();
    for axis in 0..3_usize {
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        for fixed_u in [0, n] {
            for fixed_v in [0, n] {
                for step in 0..n {
                    let (mut a, mut b) = ([0_u32; 3], [0_u32; 3]);
                    a[u] = fixed_u;
                    b[u] = fixed_u;
                    a[v] = fixed_v;
                    b[v] = fixed_v;
                    a[axis] = step;
                    b[axis] = step + 1;
                    edges.push([vertex(a), vertex(b)]);
                }
            }
        }
    }
    (positions, sizes, corners, edges)
}

fn main() {
    let sizes: Vec<u32> = std::env::args()
        .skip(1)
        .filter_map(|arg| arg.parse().ok())
        .collect();
    let sizes = if sizes.is_empty() {
        vec![50, 100]
    } else {
        sizes
    };
    for n in sizes {
        let (positions, face_sizes, corners, edges) = grid_cube(n);
        let mesh = PolygonMesh {
            positions: &positions,
            face_sizes: &face_sizes,
            corners: &corners,
        };
        let options = BevelOptions {
            width: 0.02,
            segments: 3,
            offset_type: BevelOffset::Offset,
            profile: 0.5,
        };
        let started = Instant::now();
        match bevel_edges(&mesh, &edges, &options) {
            Ok(result) => println!(
                "n={n} in: {} faces, {} edges beveled -> out: {} vertices, {} faces, {:.0} ms",
                face_sizes.len(),
                edges.len(),
                result.positions.len(),
                result.face_sizes.len(),
                started.elapsed().as_secs_f64() * 1000.0
            ),
            Err(error) => println!(
                "n={n} failed: {error} after {:.0} ms",
                started.elapsed().as_secs_f64() * 1000.0
            ),
        }
    }
}
