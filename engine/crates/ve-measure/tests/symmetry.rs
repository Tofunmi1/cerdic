//! Symmetry: a symmetric cube, a broken cube, and the best plane.

use ve_measure::{Mesh, Plane, best_symmetry_plane, symmetry_error};

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

#[test]
fn a_cube_is_symmetric_about_every_axis() {
    let (positions, indices) = cube(0.5);
    let mesh = Mesh::new(&positions, &indices);
    for plane in Plane::all() {
        let report = symmetry_error(&mesh, plane).expect("valid");
        assert!(report.max < 1e-5, "{plane:?}: {}", report.max);
        assert!(report.mean < 1e-5);
    }
}

#[test]
fn moving_one_vertex_breaks_x_symmetry() {
    let (mut positions, indices) = cube(0.5);
    positions[6][0] += 0.4;
    let mesh = Mesh::new(&positions, &indices);
    let report = symmetry_error(&mesh, Plane::X).expect("valid");
    assert!(report.max > 0.2, "{}", report.max);
}

#[test]
fn the_best_plane_is_the_least_wrong_one() {
    let (mut positions, mut indices) = cube(0.5);
    positions.push([0.0, 0.9, 0.4]);
    indices.extend_from_slice(&[0, 1, 8]);
    let mesh = Mesh::new(&positions, &indices);
    let best = best_symmetry_plane(&mesh).expect("valid");
    let mut smallest = f32::INFINITY;
    let mut largest = 0.0f32;
    for plane in Plane::all() {
        let error = symmetry_error(&mesh, plane).expect("valid").max;
        smallest = smallest.min(error);
        largest = largest.max(error);
    }
    assert!((best.max - smallest).abs() < 1e-6);
    assert!(
        largest > best.max + 1e-3,
        "at least one plane is clearly worse"
    );
}

#[test]
fn a_shape_symmetric_only_in_x_picks_x() {
    let (mut positions, mut indices) = cube(0.5);
    // A single spike on the +Z side keeps X symmetry and breaks Y and Z.
    positions.push([0.0, 0.9, 0.4]);
    indices.extend_from_slice(&[0, 1, 8]);
    let mesh = Mesh::new(&positions, &indices);
    let best = best_symmetry_plane(&mesh).expect("valid");
    assert_eq!(best.plane, Plane::X);
    assert!(best.max < 1e-4, "{}", best.max);
    assert!(symmetry_error(&mesh, Plane::Z).expect("valid").max > 0.2);
}
