//! Convex hull: a cube, a budget refusal and degenerate input.

use ve_measure::{MeasureError, Mesh, Report, convex_hull};

fn cube_points(half: f32) -> Vec<[f32; 3]> {
    let h = half;
    vec![
        [-h, -h, -h],
        [h, -h, -h],
        [h, h, -h],
        [-h, h, -h],
        [-h, -h, h],
        [h, -h, h],
        [h, h, h],
        [-h, h, h],
    ]
}

#[test]
fn the_hull_of_a_cube_is_that_cube() {
    let hull = convex_hull(&cube_points(0.5)).expect("cube points have a hull");
    assert_eq!(hull.vertex_count(), 8);
    assert!(hull.triangle_count() >= 12, "{}", hull.triangle_count());
    let report = Report::of(&Mesh::new(hull.positions(), hull.indices())).expect("hull is valid");
    assert!(report.watertight);
    assert!((report.volume - 1.0).abs() < 1e-4, "{}", report.volume);
}

#[test]
fn the_hull_ignores_interior_points() {
    let mut points = cube_points(0.5);
    points.push([0.0, 0.0, 0.0]);
    points.push([0.1, -0.1, 0.2]);
    let hull = convex_hull(&points).expect("hull");
    assert_eq!(hull.vertex_count(), 8);
}

#[test]
fn too_many_points_are_refused() {
    let count = ve_measure::MAX_HULL_POINTS + 1;
    let points = vec![[0.0f32, 0.0, 0.0]; count];
    let err = convex_hull(&points).unwrap_err();
    assert!(matches!(err, MeasureError::PointBudget { .. }));
}

#[test]
fn flat_input_is_degenerate() {
    let planar = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
    ];
    let err = convex_hull(&planar).unwrap_err();
    assert!(matches!(err, MeasureError::Degenerate(_)));
}
