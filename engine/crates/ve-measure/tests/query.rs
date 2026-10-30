//! Queries: ray casts, closest points and the sampled Hausdorff distance.

use ve_measure::{Mesh, closest_point, distance_between, raycast};

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
fn a_ray_hits_the_near_face() {
    let (positions, indices) = cube(0.5);
    let mesh = Mesh::new(&positions, &indices);
    let hit = raycast(&mesh, [-2.0, 0.0, 0.0], [1.0, 0.0, 0.0])
        .expect("valid")
        .expect("hit");
    assert!((hit.distance - 1.5).abs() < 1e-4, "{}", hit.distance);
    assert!((hit.point[0] + 0.5).abs() < 1e-4);
    // A ray that misses the cube entirely.
    assert!(
        raycast(&mesh, [-2.0, 2.0, 0.0], [1.0, 0.0, 0.0])
            .expect("valid")
            .is_none()
    );
}

#[test]
fn the_closest_point_is_on_the_surface() {
    let (positions, indices) = cube(0.5);
    let mesh = Mesh::new(&positions, &indices);
    let closest = closest_point(&mesh, [2.0, 0.0, 0.0]).expect("valid");
    assert!(
        (closest.distance - 1.5).abs() < 1e-4,
        "{}",
        closest.distance
    );
    assert!((closest.point[0] - 0.5).abs() < 1e-4);
    // A point inside the cube is half a metre from the nearest face.
    let inside = closest_point(&mesh, [0.0, 0.0, 0.0]).expect("valid");
    assert!((inside.distance - 0.5).abs() < 1e-4, "{}", inside.distance);
}

#[test]
fn identical_meshes_are_zero_apart() {
    let (positions, indices) = cube(0.5);
    let a = Mesh::new(&positions, &indices);
    let hausdorff = distance_between(&a, &a).expect("valid");
    assert!(hausdorff.max < 1e-5, "{}", hausdorff.max);
    assert!(hausdorff.mean < 1e-5);
}

#[test]
fn a_shifted_mesh_is_apart_by_the_shift() {
    let (positions, indices) = cube(0.5);
    let shifted: Vec<[f32; 3]> = positions
        .iter()
        .map(|p| [p[0] + 0.25, p[1], p[2]])
        .collect();
    let a = Mesh::new(&positions, &indices);
    let b = Mesh::new(&shifted, &indices);
    let hausdorff = distance_between(&a, &b).expect("valid");
    assert!(hausdorff.max >= 0.25 - 1e-4, "{}", hausdorff.max);
    assert!(hausdorff.mean > 0.0);
    assert!(hausdorff.mean <= hausdorff.max);
}
