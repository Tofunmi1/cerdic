//! Per-element checks for semi-sharp edge and corner interpolation.
use std::collections::BTreeSet;
use ve_kernel::{
    CornerSharpness, Crease, ExecutionContext, subdivide_catmark, subdivide_catmark_with_features,
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

fn point_with_origin(result: &ve_kernel::SubdivisionMesh, kind: u32, parent: usize) -> [f32; 3] {
    let index = result
        .vertex_origin_kind
        .iter()
        .zip(&result.vertex_origin_index)
        .position(|(&actual_kind, &actual_parent)| {
            actual_kind == kind && actual_parent as usize == parent
        });
    let Some(index) = index else {
        return [f32::NAN; 3];
    };
    result.positions[index]
}

fn mix(a: [f32; 3], b: [f32; 3], weight: f32) -> [f32; 3] {
    std::array::from_fn(|axis| a[axis] * (1.0 - weight) + b[axis] * weight)
}

#[test]
fn edge_sharpness_sweep_blends_each_parent_edge_point() {
    let context = ExecutionContext::new().unwrap();
    let (positions, sizes, corners) = cube();
    let edges: BTreeSet<_> = corners
        .chunks_exact(4)
        .flat_map(|face| {
            (0..4).map(|index| {
                let a = face[index];
                let b = face[(index + 1) % 4];
                (a.min(b), a.max(b))
            })
        })
        .collect();
    let parent_edge = edges.iter().position(|edge| *edge == (0, 1)).unwrap();
    let smooth = subdivide_catmark(&positions, &sizes, &corners, 1, 0, 0, &context).unwrap();
    let smooth_point = point_with_origin(&smooth, 1, parent_edge);
    let sharp_point = mix(positions[0], positions[1], 0.5);
    for sharpness in [0.0_f32, 0.5, 1.0, 2.0, 10.0] {
        let result = subdivide_catmark_with_features(
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
        .unwrap();
        let actual = point_with_origin(&result, 1, parent_edge);
        let expected = mix(smooth_point, sharp_point, sharpness.min(1.0));
        assert!(
            close(actual, expected),
            "edge sharpness {sharpness}: {actual:?} != {expected:?}"
        );
    }
}

#[test]
fn corner_sharpness_sweep_blends_each_parent_vertex() {
    let context = ExecutionContext::new().unwrap();
    let (positions, sizes, corners) = cube();
    let smooth = subdivide_catmark(&positions, &sizes, &corners, 1, 0, 0, &context).unwrap();
    let smooth_point = point_with_origin(&smooth, 0, 0);
    for sharpness in [0.0_f32, 0.5, 1.0, 2.0, 10.0] {
        let result = subdivide_catmark_with_features(
            &positions,
            &sizes,
            &corners,
            &[],
            &[CornerSharpness {
                vertex: 0,
                sharpness,
            }],
            1,
            0,
            0,
            &context,
        )
        .unwrap();
        let actual = point_with_origin(&result, 0, 0);
        let expected = mix(smooth_point, positions[0], sharpness.min(1.0));
        assert!(
            close(actual, expected),
            "corner sharpness {sharpness}: {actual:?} != {expected:?}"
        );
    }
}

fn close(actual: [f32; 3], expected: [f32; 3]) -> bool {
    actual
        .iter()
        .zip(expected)
        .all(|(a, b)| (*a - b).abs() < 1.0e-6)
}
