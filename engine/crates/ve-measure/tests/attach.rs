//! Attachment checks for named scene parts.
#![allow(
    clippy::cast_precision_loss,
    clippy::float_cmp,
    reason = "these tests use exact zero gaps and small bounded integer coordinates"
)]

use ve_measure::{MeasureError, Mesh, NamedMesh, attachments, attachments_within};

const INDICES: [u32; 36] = [
    0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 1, 2, 6, 1, 6, 5, 2, 3, 7, 2, 7, 6, 3, 0,
    4, 3, 4, 7,
];

fn box_mesh(min: [f32; 3], size: f32) -> Vec<[f32; 3]> {
    let [x, y, z] = min;
    vec![
        [x, y, z],
        [x + size, y, z],
        [x + size, y + size, z],
        [x, y + size, z],
        [x, y, z + size],
        [x + size, y, z + size],
        [x + size, y + size, z + size],
        [x, y + size, z + size],
    ]
}

fn part<'a>(name: &'a str, positions: &'a [[f32; 3]]) -> NamedMesh<'a> {
    NamedMesh {
        name,
        mesh: Mesh::new(positions, &INDICES),
    }
}

#[test]
fn touching_boxes_are_attached() {
    let a = box_mesh([0.0; 3], 1.0);
    let b = box_mesh([1.0, 0.0, 0.0], 1.0);
    let result = attachments(&[part("a", &a), part("b", &b)], 0.01).unwrap();
    assert_eq!(result[0].gap, 0.0);
    assert_eq!(result[1].gap, 0.0);
}

#[test]
fn a_box_inside_another_is_attached() {
    let big = box_mesh([-2.0; 3], 4.0);
    let small = box_mesh([-0.5; 3], 1.0);
    let result = attachments(&[part("big", &big), part("small", &small)], 0.01).unwrap();
    assert_eq!(result[0].gap, 0.0);
    assert_eq!(result[1].gap, 0.0);
}

#[test]
fn a_floating_box_reports_its_gap() {
    let a = box_mesh([0.0; 3], 1.0);
    let b = box_mesh([2.5, 0.0, 0.0], 1.0);
    let result = attachments(&[part("a", &a), part("b", &b)], 0.01).unwrap();
    for item in result {
        assert!((item.gap - 1.5).abs() < 1e-4, "{}", item.gap);
    }
    assert_eq!(
        attachments(&[part("a", &a), part("b", &b)], 0.01).unwrap()[0].nearest,
        "b"
    );
}

#[test]
fn three_parts_the_far_one_floats() {
    let a = box_mesh([0.0; 3], 1.0);
    let b = box_mesh([1.0, 0.0, 0.0], 1.0);
    let c = box_mesh([2.4, 0.0, 0.0], 1.0);
    let result = attachments(&[part("a", &a), part("b", &b), part("c", &c)], 0.01).unwrap();
    assert_eq!(result[0].gap, 0.0);
    assert_eq!(result[1].gap, 0.0);
    assert!((result[2].gap - 0.4).abs() < 1e-4);
}

#[test]
fn tolerance_decides() {
    let a = box_mesh([0.0; 3], 1.0);
    let b = box_mesh([1.0, 0.0, 0.0], 1.0);
    let c = box_mesh([2.4, 0.0, 0.0], 1.0);
    let result = attachments(&[part("a", &a), part("b", &b), part("c", &c)], 0.5).unwrap();
    assert!(result.iter().all(|item| item.gap <= 0.5));
}

#[test]
fn too_many_parts_are_refused() {
    let boxes: Vec<_> = (0..65)
        .map(|i| box_mesh([i as f32 * 2.0, 0.0, 0.0], 0.1))
        .collect();
    let parts: Vec<_> = boxes
        .iter()
        .map(|p| NamedMesh {
            name: "tiny",
            mesh: Mesh::new(p, &INDICES),
        })
        .collect();
    let error = attachments(&parts, 0.01).unwrap_err();
    assert!(matches!(error, MeasureError::PartBudget { .. }));
}

/// A flat grid of `cells` x `cells` quads from (x0, 0, 0) to (x0 + 1, 0, 1): many vertices, two triangles a cell.
fn grid(x0: f32, cells: usize) -> (Vec<[f32; 3]>, Vec<u32>) {
    let side = cells + 1;
    let positions = (0..side * side)
        .map(|k| [x0 + (k % side) as f32 / cells as f32, 0.0, (k / side) as f32 / cells as f32])
        .collect();
    let mut indices = Vec::new();
    for row in 0..cells {
        for col in 0..cells {
            let a = (row * side + col) as u32;
            let (b, c, d) = (a + 1, a + side as u32, a + side as u32 + 1);
            indices.extend([a, b, d, a, d, c]);
        }
    }
    (positions, indices)
}

#[test]
fn the_bounded_check_finds_the_same_floating_parts_as_the_exact_one() {
    let a = box_mesh([0.0; 3], 1.0);
    let b = box_mesh([1.0, 0.0, 0.0], 1.0);
    let c = box_mesh([4.0, 0.0, 0.0], 1.0);
    let d = box_mesh([0.25; 3], 0.5);
    let parts = [part("a", &a), part("b", &b), part("c", &c), part("d", &d)];
    let exact = attachments(&parts, 0.01).unwrap();
    let bounded = attachments_within(&parts, 0.01, 1_000_000).unwrap();
    for (e, b) in exact.iter().zip(&bounded) {
        assert_eq!(e.name, b.name);
        assert_eq!(e.gap > 0.01, b.gap > 0.01, "{}: exact {} bounded {}", e.name, e.gap, b.gap);
    }
    let c_gap = bounded.iter().find(|item| item.name == "c").unwrap();
    assert!((c_gap.gap - 2.0).abs() < 1e-4, "a floating part keeps its exact gap: {}", c_gap.gap);
    assert_eq!(c_gap.nearest, "b");
}

#[test]
fn attached_parts_stop_the_search_early_so_a_big_scene_stays_cheap() {
    // 20 fine grids side by side, each sharing an edge with the next: every part is attached to a neighbour at its first vertices.
    let grids: Vec<_> = (0..20).map(|i| grid(i as f32, 40)).collect();
    let parts: Vec<_> = grids
        .iter()
        .map(|(positions, indices)| NamedMesh { name: "tile", mesh: Mesh::new(positions, indices) })
        .collect();
    let measured = attachments_within(&parts, 0.01, 5_000).unwrap();
    assert!(measured.iter().all(|item| item.gap <= 0.01), "every tile touches a neighbour");
}

#[test]
fn a_hopeless_scene_is_refused_by_the_budget_instead_of_running_for_minutes() {
    let (pa, ia) = grid(0.0, 40);
    let (pb, ib) = grid(50.0, 40);
    let parts = [
        NamedMesh { name: "near", mesh: Mesh::new(&pa, &ia) },
        NamedMesh { name: "far", mesh: Mesh::new(&pb, &ib) },
    ];
    let error = attachments_within(&parts, 0.01, 100).unwrap_err();
    assert!(matches!(error, MeasureError::WorkBudget { limit: 100 }), "{error:?}");
}

#[test]
fn a_single_part_has_no_neighbour() {
    let a = box_mesh([0.0; 3], 1.0);
    let result = attachments(&[part("alone", &a)], 0.01).unwrap();
    assert_eq!(result[0].gap, 0.0);
    assert!(result[0].nearest.is_empty());
}
