//! Build a cube and a torus in code and print their report summaries.
//!
//! Run with `cargo run -p ve-measure --example report`.
#![expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "the example grid indexes and scales with small counts"
)]

use ve_measure::{Mesh, Report};

fn cube(half: f32) -> (Vec<[f32; 3]>, Vec<u32>) {
    let h = half;
    let positions = vec![
        [-h, -h, -h],
        [h, -h, -h],
        [h, h, -h],
        [-h, h, -h],
        [-h, -h, h],
        [h, -h, h],
        [h, h, h],
        [-h, h, h],
    ];
    let indices = vec![
        0, 3, 2, 0, 2, 1, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 3, 7, 6, 3, 6, 2, 0, 4, 7, 0, 7, 3,
        1, 2, 6, 1, 6, 5,
    ];
    (positions, indices)
}

fn torus(major: f32, minor: f32, nu: usize, nv: usize) -> (Vec<[f32; 3]>, Vec<u32>) {
    let mut positions = Vec::with_capacity(nu * nv);
    for i in 0..nu {
        let u = i as f32 / nu as f32 * std::f32::consts::TAU;
        for j in 0..nv {
            let v = j as f32 / nv as f32 * std::f32::consts::TAU;
            let ring = major + minor * v.cos();
            positions.push([ring * u.cos(), minor * v.sin(), ring * u.sin()]);
        }
    }
    let mut indices = Vec::new();
    for i in 0..nu {
        for j in 0..nv {
            let a = (i * nv + j) as u32;
            let b = ((i + 1) % nu * nv + j) as u32;
            let c = ((i + 1) % nu * nv + (j + 1) % nv) as u32;
            let d = (i * nv + (j + 1) % nv) as u32;
            indices.extend_from_slice(&[a, c, b, a, d, c]);
        }
    }
    (positions, indices)
}

#[expect(
    clippy::print_stdout,
    reason = "the example prints its summaries to stdout"
)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (cube_positions, cube_indices) = cube(0.5);
    let cube = Report::of(&Mesh::new(&cube_positions, &cube_indices))?;
    println!("== cube ==");
    println!("{}", cube.summary());

    let (torus_positions, torus_indices) = torus(1.0, 0.35, 24, 16);
    let torus = Report::of(&Mesh::new(&torus_positions, &torus_indices))?;
    println!();
    println!("== torus ==");
    println!("{}", torus.summary());
    Ok(())
}
