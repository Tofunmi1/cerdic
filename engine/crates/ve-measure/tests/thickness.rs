//! Wall thickness: a hollow shell, a solid sphere and the triangle budget.
#![expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "the test geometry indexes and scales with small counts"
)]

use ve_measure::{MeasureError, Mesh, min_wall_thickness};

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

/// A closed shell: an outer cube and an inner cube wound the other way.
fn shell(wall: f32) -> (Vec<[f32; 3]>, Vec<u32>) {
    let (mut positions, outer) = cube(0.5);
    let (inner_positions, inner) = cube(0.5 - wall);
    let offset = positions.len() as u32;
    positions.extend(inner_positions);
    let mut indices = outer;
    let flipped: Vec<u32> = inner
        .chunks(3)
        .flat_map(|t| [t[0] + offset, t[2] + offset, t[1] + offset])
        .collect();
    indices.extend(flipped);
    (positions, indices)
}

fn sphere(radius: f32) -> (Vec<[f32; 3]>, Vec<u32>) {
    let (nu, nv) = (24usize, 12usize);
    let mut positions = Vec::new();
    for i in 0..nu {
        let u = i as f32 / nu as f32 * std::f32::consts::TAU;
        for j in 0..nv {
            let v = j as f32 / nv as f32 * std::f32::consts::PI;
            positions.push([
                radius * v.sin() * u.cos(),
                radius * v.cos(),
                radius * v.sin() * u.sin(),
            ]);
        }
    }
    let mut indices = Vec::new();
    for i in 0..nu {
        for j in 0..nv - 1 {
            let a = (i * nv + j) as u32;
            let b = ((i + 1) % nu * nv + j) as u32;
            let c = ((i + 1) % nu * nv + j + 1) as u32;
            let d = (i * nv + j + 1) as u32;
            indices.extend_from_slice(&[a, b, c, a, c, d]);
        }
    }
    (positions, indices)
}

#[test]
fn a_two_centimetre_shell_measures_two_centimetres() {
    let (positions, indices) = shell(0.02);
    let mesh = Mesh::new(&positions, &indices);
    let thickness = min_wall_thickness(&mesh, 800).expect("the shell has walls");
    assert!(
        (thickness.min - 0.02).abs() < 0.002,
        "min = {}",
        thickness.min
    );
    assert!(thickness.samples_hit > 0);
    assert!(thickness.worst.len() <= 8);
    assert!(thickness.percentile_5 >= thickness.min);
}

#[test]
fn a_solid_sphere_has_positive_thickness() {
    let (positions, indices) = sphere(0.5);
    let thickness = min_wall_thickness(&Mesh::new(&positions, &indices), 400).expect("sphere");
    assert!(thickness.min > 0.0);
    assert!(thickness.min <= 1.0 + 1e-3);
}

#[test]
fn an_open_surface_has_no_wall() {
    let positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let indices = vec![0, 1, 2];
    let err = min_wall_thickness(&Mesh::new(&positions, &indices), 64).unwrap_err();
    assert_eq!(err, MeasureError::NoThickness);
}

#[test]
fn zero_samples_is_refused() {
    let (positions, indices) = cube(0.5);
    let err = min_wall_thickness(&Mesh::new(&positions, &indices), 0).unwrap_err();
    assert!(matches!(err, MeasureError::Degenerate(_)));
}

#[test]
fn the_triangle_budget_is_a_typed_error() {
    // A mesh with more triangles than the budget, sharing three vertices, so it costs little.
    let positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let count = ve_measure::MAX_TRIANGLES + 1;
    let indices = vec![0u32; count * 3];
    let err = min_wall_thickness(&Mesh::new(&positions, &indices), 1).unwrap_err();
    assert!(matches!(err, MeasureError::TriangleBudget { .. }));
}
