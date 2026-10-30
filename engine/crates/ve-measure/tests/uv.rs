//! UV quality: a perfect unit square, two overlapping islands and out-of-range uvs.

use ve_measure::{MeasureError, Mesh, uv_report};

fn approx(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() < eps
}

#[test]
fn a_unit_square_unwrap_is_perfect() {
    let positions = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
    ];
    let indices = vec![0, 1, 2, 0, 2, 3];
    let uvs = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let mesh = Mesh::with_uvs(&positions, &indices, &uvs);
    let report = uv_report(&mesh).expect("valid");
    assert_eq!(report.islands, 1);
    assert!(
        approx(report.stretch_max, 0.0, 1e-4),
        "{}",
        report.stretch_max
    );
    assert!(approx(report.stretch_mean, 0.0, 1e-4));
    assert!(
        approx(report.texel_density_min, 1.0, 1e-3),
        "{}",
        report.texel_density_min
    );
    assert!(approx(report.texel_density_max, 1.0, 1e-3));
    assert!(approx(report.coverage, 1.0, 0.02), "{}", report.coverage);
    assert!(approx(report.overlapping_area_ratio, 0.0, 1e-6));
    assert_eq!(report.out_of_range, 0);
}

#[test]
fn two_overlapping_islands_are_detected() {
    // Two separate world quads unwrapped into overlapping UV regions.
    let positions = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [4.0, 0.0, 0.0],
        [5.0, 0.0, 0.0],
        [5.0, 1.0, 0.0],
        [4.0, 1.0, 0.0],
    ];
    let indices = vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7];
    let uvs = vec![
        [0.0, 0.0],
        [1.0, 0.0],
        [1.0, 1.0],
        [0.0, 1.0],
        [0.5, 0.0],
        [1.5, 0.0],
        [1.5, 1.0],
        [0.5, 1.0],
    ];
    let mesh = Mesh::with_uvs(&positions, &indices, &uvs);
    let report = uv_report(&mesh).expect("valid");
    assert_eq!(report.islands, 2);
    assert!(
        report.overlapping_area_ratio > 0.2,
        "{}",
        report.overlapping_area_ratio
    );
    assert!(report.overlapping_area_ratio < 0.6);
    assert_eq!(report.out_of_range, 2);
}

#[test]
fn a_mesh_without_uvs_is_a_typed_error() {
    let positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let indices = vec![0, 1, 2];
    assert_eq!(
        uv_report(&Mesh::new(&positions, &indices)),
        Err(MeasureError::MissingUv)
    );
}
