//! Acceptance tests for the CPU Catmull-Clark kernel adapter.
#![expect(
    clippy::cast_precision_loss,
    reason = "the independent reference fixture uses face sizes and valences below eight"
)]
use std::collections::BTreeMap;
use ve_kernel::{
    Crease, ExecutionContext, KernelError, subdivide_catmark, subdivide_catmark_with_features,
};

fn cube() -> (Vec<[f32; 3]>, Vec<u32>, Vec<u32>) {
    let positions = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
        [0.0, 1.0, 1.0],
    ];
    let corners = vec![
        0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 3, 7, 6, 2, 0, 4, 7, 3, 1, 2, 6, 5,
    ];
    (positions, vec![4; 6], corners)
}

fn add_scaled(target: &mut [f32; 3], value: [f32; 3], scale: f32) {
    for axis in 0..3 {
        target[axis] += value[axis] * scale;
    }
}

fn face_points(positions: &[[f32; 3]], sizes: &[u32], corners: &[u32]) -> Vec<[f32; 3]> {
    let mut offset = 0;
    sizes
        .iter()
        .map(|&size| {
            let mut point = [0.0; 3];
            for &index in &corners[offset..offset + size as usize] {
                add_scaled(&mut point, positions[index as usize], 1.0 / size as f32);
            }
            offset += size as usize;
            point
        })
        .collect()
}

fn reference_closed_catmark(
    positions: &[[f32; 3]],
    sizes: &[u32],
    corners: &[u32],
) -> Vec<[f32; 3]> {
    let faces = face_points(positions, sizes, corners);
    let mut edges = BTreeMap::<(u32, u32), Vec<usize>>::new();
    let mut offset = 0;
    for (face, &size) in sizes.iter().enumerate() {
        let loop_indices = &corners[offset..offset + size as usize];
        for local in 0..loop_indices.len() {
            let pair = (
                loop_indices[local].min(loop_indices[(local + 1) % loop_indices.len()]),
                loop_indices[local].max(loop_indices[(local + 1) % loop_indices.len()]),
            );
            edges.entry(pair).or_default().push(face);
        }
        offset += size as usize;
    }
    let edge_points: Vec<_> = edges
        .keys()
        .map(|&(a, b)| {
            let mut point = [0.0; 3];
            add_scaled(&mut point, positions[a as usize], 0.25);
            add_scaled(&mut point, positions[b as usize], 0.25);
            for face in &edges[&(a, b)] {
                add_scaled(&mut point, faces[*face], 0.25);
            }
            point
        })
        .collect();
    let mut vertex_faces = vec![Vec::new(); positions.len()];
    let mut vertex_edges = vec![Vec::new(); positions.len()];
    for (face, &size) in sizes.iter().enumerate() {
        let face_loop =
            &corners[offset_for_face(sizes, face)..offset_for_face(sizes, face) + size as usize];
        for &vertex in face_loop {
            vertex_faces[vertex as usize].push(face);
        }
    }
    for &(a, b) in edges.keys() {
        vertex_edges[a as usize].push((a, b));
        vertex_edges[b as usize].push((a, b));
    }
    let mut result = faces;
    result.extend(edge_points);
    for (vertex, original) in positions.iter().enumerate() {
        let n = vertex_faces[vertex].len() as f32;
        let mut f_average = [0.0; 3];
        for face in &vertex_faces[vertex] {
            add_scaled(&mut f_average, result[*face], 1.0 / n);
        }
        let mut r_average = [0.0; 3];
        for &(a, b) in &vertex_edges[vertex] {
            let midpoint = [
                (positions[a as usize][0] + positions[b as usize][0]) * 0.5,
                (positions[a as usize][1] + positions[b as usize][1]) * 0.5,
                (positions[a as usize][2] + positions[b as usize][2]) * 0.5,
            ];
            add_scaled(&mut r_average, midpoint, 1.0 / n);
        }
        let mut point = [0.0; 3];
        add_scaled(&mut point, f_average, 1.0 / n);
        add_scaled(&mut point, r_average, 2.0 / n);
        add_scaled(&mut point, *original, (n - 3.0) / n);
        result.push(point);
    }
    result
}

fn offset_for_face(sizes: &[u32], face: usize) -> usize {
    sizes[..face].iter().map(|size| *size as usize).sum()
}

fn assert_parent_reference(
    result: &ve_kernel::SubdivisionMesh,
    reference: &[[f32; 3]],
    source_vertices: usize,
    source_faces: usize,
    label: &str,
) {
    let source_edges = reference.len() - source_vertices - source_faces;
    for (index, actual) in result.positions.iter().enumerate() {
        let parent = result.vertex_origin_index[index] as usize;
        let kind = result.vertex_origin_kind[index];
        assert!(kind <= 2, "unexpected parent kind {kind}");
        let reference_index = match kind {
            0 => source_faces + source_edges + parent,
            1 => source_faces + parent,
            _ => parent,
        };
        let expected = reference[reference_index];
        let error = (0..3)
            .map(|axis| (actual[axis] - expected[axis]).powi(2))
            .sum::<f32>();
        assert!(
            error < 1.0e-12,
            "{label}, vertex {index}: {actual:?} != {expected:?}"
        );
    }
}

#[test]
fn cube_refines_to_26_vertices_and_24_quads_with_provenance() {
    let context = ExecutionContext::new().unwrap();
    let (positions, sizes, corners) = cube();
    let result = subdivide_catmark(&positions, &sizes, &corners, 1, 0, 0, &context).unwrap();
    assert_eq!(
        (
            result.positions.len(),
            result.face_sizes.len(),
            result.corners.len()
        ),
        (26, 24, 96)
    );
    assert!(result.face_sizes.iter().all(|&size| size == 4));
    assert_eq!(
        result
            .vertex_origin_kind
            .iter()
            .filter(|&&kind| kind == 0)
            .count(),
        8
    );
    assert_eq!(
        result
            .vertex_origin_kind
            .iter()
            .filter(|&&kind| kind == 1)
            .count(),
        12
    );
    assert_eq!(
        result
            .vertex_origin_kind
            .iter()
            .filter(|&&kind| kind == 2)
            .count(),
        6
    );
    assert!(
        result
            .face_parent
            .iter()
            .enumerate()
            .all(|(face, parent)| *parent as usize == face / 4)
    );
    for point in &result.positions {
        assert!((0.0..=1.0).contains(&point[0]));
        assert!((0.0..=1.0).contains(&point[1]));
        assert!((0.0..=1.0).contains(&point[2]));
    }
    let expected = reference_closed_catmark(&positions, &sizes, &corners);
    assert_eq!(expected.len(), result.positions.len());
    assert_parent_reference(&result, &expected, positions.len(), sizes.len(), "cube");
}

#[test]
fn repeated_cube_refinement_stays_bounded_and_its_extent_converges() {
    let context = ExecutionContext::new().unwrap();
    let (mut positions, mut sizes, mut corners) = cube();
    let mut previous_span = [1.0_f32; 3];
    for level in 1..=4 {
        let refined = subdivide_catmark(&positions, &sizes, &corners, 1, 0, 0, &context).unwrap();
        let mut minimum = [f32::INFINITY; 3];
        let mut maximum = [f32::NEG_INFINITY; 3];
        for point in &refined.positions {
            for axis in 0..3 {
                minimum[axis] = minimum[axis].min(point[axis]);
                maximum[axis] = maximum[axis].max(point[axis]);
            }
        }
        let span = std::array::from_fn::<_, 3, _>(|axis| maximum[axis] - minimum[axis]);
        for axis in 0..3 {
            assert!(
                span[axis] <= previous_span[axis] + 1.0e-6,
                "level {level}: {span:?}"
            );
            assert!(span[axis] > 0.0);
        }
        previous_span = span;
        positions = refined.positions;
        sizes = refined.face_sizes;
        corners = refined.corners;
    }
    assert!(previous_span.iter().all(|&span| span < 1.0));
}

#[test]
fn open_quad_boundary_rule_preserves_boundary_corners_when_requested() {
    let context = ExecutionContext::new().unwrap();
    let positions = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
    ];
    let sizes = [4];
    let corners = [0, 1, 2, 3];
    let output = subdivide_catmark(&positions, &sizes, &corners, 1, 2, 0, &context).unwrap();
    assert_eq!((output.positions.len(), output.face_sizes.len()), (9, 4));
    for (origin, expected) in positions.iter().enumerate() {
        let index = output
            .vertex_origin_kind
            .iter()
            .zip(&output.vertex_origin_index)
            .position(|(kind, parent)| *kind == 0 && *parent as usize == origin);
        let output_point = index.map(|vertex| output.positions[vertex]);
        assert_eq!(output_point, Some(*expected));
    }
    let edge_only = subdivide_catmark(&positions, &sizes, &corners, 1, 1, 0, &context).unwrap();
    assert!(edge_only.positions.iter().any(|point| {
        (point[0] - 0.125).abs() < 1.0e-6
            && (point[1] - 0.125).abs() < 1.0e-6
            && point[2].abs() < 1.0e-6
    }));
}

#[test]
fn integral_edge_sharpness_zero_and_full_match_the_expected_endpoints() {
    let context = ExecutionContext::new().unwrap();
    let (positions, sizes, corners) = cube();
    let run = |sharpness| {
        subdivide_catmark_with_features(
            &positions,
            &sizes,
            &corners,
            &[Crease {
                vertices: [0, 1],
                sharpness,
            }],
            &[],
            1,
            0,
            0,
            &context,
        )
        .unwrap()
    };
    let smooth = subdivide_catmark(&positions, &sizes, &corners, 1, 0, 0, &context).unwrap();
    assert_eq!(run(0.0).positions, smooth.positions);
    let sharp = run(1.0).positions;
    assert_eq!(run(2.0).positions, sharp);
    assert_eq!(run(10.0).positions, sharp);
}

#[test]
fn extraordinary_vertices_of_valence_three_and_five_refine_finitely() {
    let context = ExecutionContext::new().unwrap();
    for valence in [3_u32, 5] {
        let (positions, sizes, corners) = bipyramid(valence);
        let result = subdivide_catmark(&positions, &sizes, &corners, 1, 0, 0, &context).unwrap();
        assert_eq!(result.face_sizes.len(), corners.len());
        assert!(
            result
                .positions
                .iter()
                .flatten()
                .all(|value| value.is_finite())
        );
        assert!(
            result
                .positions
                .iter()
                .flatten()
                .all(|value| (-1.0..=1.0).contains(value))
        );
        let expected = reference_closed_catmark(&positions, &sizes, &corners);
        assert_eq!(expected.len(), result.positions.len());
        assert_parent_reference(
            &result,
            &expected,
            positions.len(),
            sizes.len(),
            &format!("valence {valence}"),
        );
    }
}

fn bipyramid(valence: u32) -> (Vec<[f32; 3]>, Vec<u32>, Vec<u32>) {
    let mut positions = vec![[0.0, 1.0, 0.0], [0.0, -1.0, 0.0]];
    for index in 0..valence {
        let angle = std::f32::consts::TAU * index as f32 / valence as f32;
        positions.push([angle.cos(), 0.0, angle.sin()]);
    }
    let mut corners = Vec::with_capacity((valence * 6) as usize);
    for index in 0..valence {
        let next = (index + 1) % valence;
        let current = index + 2;
        let following = next + 2;
        corners.extend([0, current, following, 1, following, current]);
    }
    (positions, vec![3; (valence * 2) as usize], corners)
}

#[test]
fn refuses_nonmanifold_cage_and_pre_cancelled_work() {
    let context = ExecutionContext::new().unwrap();
    let (positions, _, _) = cube();
    let nonmanifold = subdivide_catmark(
        &positions[..4],
        &[3, 3, 3],
        &[0, 1, 2, 0, 1, 3, 0, 1, 2],
        1,
        0,
        0,
        &context,
    );
    assert_eq!(nonmanifold, Err(KernelError::NotManifold));
    context.cancel();
    assert_eq!(
        subdivide_catmark(
            &positions,
            &[4; 6],
            &[
                0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 3, 7, 6, 2, 0, 4, 7, 3, 1, 2, 6, 5
            ],
            1,
            0,
            0,
            &context
        ),
        Err(KernelError::Cancelled)
    );
}
