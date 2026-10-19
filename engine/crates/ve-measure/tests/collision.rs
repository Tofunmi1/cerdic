//! Collision shapes: box fits and a convex decomposition.
#![expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "the test geometry indexes and scales with small counts"
)]

use ve_measure::{Mesh, ShapeKind, bounding_shapes, convex_decomposition};

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
fn a_cube_fits_a_box_about_once_its_own_volume() {
    let (positions, indices) = cube(0.5);
    let shapes = bounding_shapes(&Mesh::new(&positions, &indices)).expect("valid");
    assert!(
        (shapes.aabb.volume - 1.0).abs() < 1e-4,
        "{}",
        shapes.aabb.volume
    );
    assert!((shapes.aabb.ratio - 1.0).abs() < 1e-3);
    assert!(
        (shapes.obb.ratio - 1.0).abs() < 1e-2,
        "{}",
        shapes.obb.ratio
    );
    assert!(shapes.capsule.volume > 1.0);
    assert!(matches!(shapes.best, ShapeKind::Aabb | ShapeKind::Obb));
}

/// A closed cylinder along +X: radius `r`, length `length`, `segments` around.
fn cylinder(r: f32, length: f32, segments: usize) -> (Vec<[f32; 3]>, Vec<u32>) {
    let half = length * 0.5;
    let mut positions = Vec::new();
    for i in 0..segments {
        let angle = i as f32 / segments as f32 * std::f32::consts::TAU;
        let (y, z) = (r * angle.cos(), r * angle.sin());
        positions.push([-half, y, z]);
        positions.push([half, y, z]);
    }
    let centre0 = positions.len() as u32;
    positions.push([-half, 0.0, 0.0]);
    let centre1 = positions.len() as u32;
    positions.push([half, 0.0, 0.0]);
    let mut indices = Vec::new();
    for i in 0..segments {
        let next = (i + 1) % segments;
        let (a0, a1) = ((i * 2) as u32, (next * 2) as u32);
        let (b0, b1) = (a0 + 1, a1 + 1);
        indices.extend_from_slice(&[a0, b1, b0, a0, a1, b1]);
        indices.extend_from_slice(&[centre0, a1, a0, centre1, b0, b1]);
    }
    (positions, indices)
}

#[test]
fn a_stretched_shape_prefers_the_capsule() {
    // A long thin cylinder: a capsule hugs it much better than a box.
    let (positions, indices) = cylinder(0.1, 2.0, 24);
    let shapes = bounding_shapes(&Mesh::new(&positions, &indices)).expect("valid");
    assert!(
        shapes.capsule.ratio < shapes.aabb.ratio,
        "capsule {} vs aabb {}",
        shapes.capsule.ratio,
        shapes.aabb.ratio
    );
    assert_eq!(shapes.best, ShapeKind::Capsule);
}

#[test]
fn a_cube_decomposes_into_bounded_parts() {
    let (positions, indices) = cube(0.5);
    let decomposition =
        convex_decomposition(&Mesh::new(&positions, &indices), 4).expect("cube has an interior");
    assert_eq!(decomposition.parts.len(), 4);
    assert!(decomposition.max_vertices <= 64);
    assert!(decomposition.max_vertices >= 4);
    for part in &decomposition.parts {
        assert!(part.hull.vertex_count() <= 64);
        assert!(part.hull.triangle_count() >= 4);
    }
}

#[test]
fn a_single_part_is_allowed() {
    let (positions, indices) = cube(0.5);
    let decomposition =
        convex_decomposition(&Mesh::new(&positions, &indices), 1).expect("interior");
    assert_eq!(decomposition.parts.len(), 1);
    assert!(decomposition.parts[0].hull.vertex_count() <= 64);
}

#[test]
fn zero_parts_is_refused() {
    let (positions, indices) = cube(0.5);
    let err = convex_decomposition(&Mesh::new(&positions, &indices), 0).unwrap_err();
    assert!(matches!(err, ve_measure::MeasureError::Degenerate(_)));
}
