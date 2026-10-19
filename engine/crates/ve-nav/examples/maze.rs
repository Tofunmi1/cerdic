//! Builds a small zigzag maze from triangles, finds a path across it and prints the navigation
//! grid as ASCII art.
//!
//! ```text
//! cargo run -p ve-nav --example maze
//! ```
#![expect(clippy::print_stdout, reason = "an example prints its maze")]
#![expect(
    clippy::expect_used,
    clippy::cast_precision_loss,
    reason = "an example reports failure by stopping and draws in cell counts"
)]

use ve_nav::{NavConfig, NavMesh, Path};

fn main() {
    let (vertices, indices) = maze();
    let config = NavConfig::new(
        0.25,
        0.25,
        1.8,
        0.4,
        45.0,
        [0.0, -1.0, 0.0],
        [10.0, 3.0, 10.0],
    )
    .expect("valid config");
    let nav = NavMesh::build(&vertices, &indices, &config).expect("maze should build");
    let path = nav
        .find_path([1.0, 0.0, 5.0], [9.0, 0.0, 5.0])
        .expect("the maze should have a route");
    print_grid(&nav, &path);
    println!("\n{} corners, {:.2} m", path.len(), path.length());
}

/// A 10 x 10 room with two walls forming a zigzag: over the first wall, then under the second.
fn maze() -> (Vec<[f32; 3]>, Vec<u32>) {
    let mut soup = Soup::default();
    soup.floor([0.0, 0.0], [10.0, 10.0], 0.0);
    soup.wall(3.0, 0.0, 7.0);
    soup.wall(6.0, 3.0, 10.0);
    (soup.vertices, soup.indices)
}

#[derive(Default)]
struct Soup {
    vertices: Vec<[f32; 3]>,
    indices: Vec<u32>,
}

impl Soup {
    fn quad(&mut self, a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3]) {
        let base = u32::try_from(self.vertices.len()).expect("maze fits in u32 indices");
        self.vertices.extend_from_slice(&[a, b, c, d]);
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    fn floor(&mut self, min: [f32; 2], max: [f32; 2], y: f32) {
        self.quad(
            [min[0], y, min[1]],
            [max[0], y, min[1]],
            [max[0], y, max[1]],
            [min[0], y, max[1]],
        );
    }

    /// A wall at `x`, running from `z0` to `z1`, `height` metres tall.
    fn wall(&mut self, x: f32, z0: f32, z1: f32) {
        self.quad([x, 0.0, z0], [x, 0.0, z1], [x, 2.5, z1], [x, 2.5, z0]);
    }
}

fn print_grid(nav: &NavMesh, path: &Path) {
    let (nx, nz) = nav.dimensions();
    let cell = nav.config().cell_size();
    let min = nav.config().bounds_min();
    println!("W = wall, * = path, . = walkable, space = blocked");
    for iz in (0..nz).rev() {
        let mut line = String::with_capacity(nx);
        for ix in 0..nx {
            let x = min[0] + (ix as f32 + 0.5) * cell;
            let z = min[2] + (iz as f32 + 0.5) * cell;
            let p = [x, 0.0, z];
            let ch = if !nav.is_walkable(p) {
                ' '
            } else if near(path, p, cell) {
                '*'
            } else {
                '.'
            };
            line.push(ch);
        }
        println!("{line}");
    }
}

fn near(path: &Path, p: [f32; 3], cell: f32) -> bool {
    path.points().windows(2).any(|w| {
        let d = distance_to_segment(p, w[0], w[1]);
        d <= cell * 0.75
    })
}

fn distance_to_segment(p: [f32; 3], a: [f32; 3], b: [f32; 3]) -> f32 {
    let (px, pz) = (p[0], p[2]);
    let (ax, az) = (a[0], a[2]);
    let (bx, bz) = (b[0], b[2]);
    let (dx, dz) = (bx - ax, bz - az);
    let length_sq = dx * dx + dz * dz;
    if length_sq <= 1.0e-9 {
        return ((px - ax).powi(2) + (pz - az).powi(2)).sqrt();
    }
    let t = (((px - ax) * dx + (pz - az) * dz) / length_sq).clamp(0.0, 1.0);
    let cx = ax + dx * t;
    let cz = az + dz * t;
    ((px - cx).powi(2) + (pz - cz).powi(2)).sqrt()
}
