//! The quad-grid source mesh and the individual-faces modes of inset and extrude (the facade chain).
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "fixture setup and assertions should stop at their first failure"
)]

use ve_geometry::{
    Element, GeometryError, MAX_INDIVIDUAL_FACES, extrude_each, grid, inset_each, validate,
};
use ve_kernel::ExecutionContext;

fn context() -> ExecutionContext {
    ExecutionContext::new().unwrap()
}

fn code<T>(result: &Result<T, GeometryError>) -> &'static str {
    match result {
        Err(GeometryError::PolyRefusal { code, .. }) => code,
        _ => panic!("expected a refusal"),
    }
}

#[test]
fn g6_41_a_grid_is_an_open_quad_sheet_facing_plus_z() {
    let wall = grid(6.0, 4.0, 6, 4).unwrap();
    assert_eq!(wall.face_count(), 24);
    assert_eq!(wall.positions().len(), 7 * 5);
    let report = validate(&wall, &context()).unwrap();
    assert_eq!(report.euler_characteristic, 1, "a disc");
    assert_eq!(report.boundary_loops, 1);
    assert!(report.inconsistent_winding_edges.is_empty());
    let (low, high) =
        wall.positions()
            .iter()
            .fold(([f32::MAX; 3], [f32::MIN; 3]), |(lo, hi), p| {
                (
                    [lo[0].min(p[0]), lo[1].min(p[1]), lo[2].min(p[2])],
                    [hi[0].max(p[0]), hi[1].max(p[1]), hi[2].max(p[2])],
                )
            });
    assert!((low[0] + 3.0).abs() < 1.0e-6 && (high[0] - 3.0).abs() < 1.0e-6);
    // centred on x = 0 and on y = 0 (the grid is 6 wide and 4 tall)
    assert!((low[1] + 2.0).abs() < 1.0e-6 && (high[1] - 2.0).abs() < 1.0e-6);
    // The first cell winds counter-clockwise seen from +z: its normal points to +z.
    let p = |i: usize| wall.positions()[wall.corners()[i] as usize];
    let (a, b, c) = (p(0), p(1), p(2));
    let normal_z = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    assert!(normal_z > 0.0);
}

#[test]
fn g6_41_bad_grids_are_refused() {
    for (width, height, columns, rows) in [
        (0.0, 1.0, 2, 2),
        (1.0, f32::NAN, 2, 2),
        (1.0, 1.0, 0, 2),
        (1.0, 1.0, 2, 0),
        (1.0, 1.0, 1000, 1000),
    ] {
        assert_eq!(
            code(&grid(width, height, columns, rows)),
            "poly.grid_invalid"
        );
    }
}

#[test]
fn g6_41_the_facade_chain_insets_every_cell_then_extrudes_the_inner_caps() {
    let wall = grid(6.0, 4.0, 6, 4).unwrap();
    let cells: Vec<usize> = (0..24).collect();
    let inset = inset_each(&wall, &cells, 0.15, &context()).unwrap();
    assert_eq!(inset.mesh.face_count(), 24 + 24 * 4);
    assert_eq!(inset.caps.len(), 24);
    // The ring and cap replace each cell in place, so the caps move; `caps` says where they went.
    let mut distinct = inset.caps.clone();
    distinct.dedup();
    assert_eq!(distinct.len(), 24);
    let windows = extrude_each(&inset.mesh, &inset.caps, [0.0, 0.0, 0.1], &context()).unwrap();
    assert_eq!(windows.face_count(), inset.mesh.face_count() + 24 * 4);
    let report = validate(&windows, &context()).unwrap();
    assert_eq!(report.euler_characteristic, 1);
    assert_eq!(report.boundary_loops, 1);
    assert!(report.nonmanifold_edges.is_empty());
    assert!(report.inconsistent_winding_edges.is_empty());
    assert!(
        windows
            .positions()
            .iter()
            .any(|p| (p[2] - 0.1).abs() < 1.0e-6),
        "the window caps stand out from the wall"
    );
}

#[test]
fn g6_41_one_bad_face_refuses_the_whole_command_naming_it() {
    let wall = grid(2.0, 2.0, 2, 2).unwrap();
    // Insetting by more than half a cell cannot work for any face; the first is named.
    let result = inset_each(&wall, &[3, 1], 0.9, &context());
    assert!(matches!(
        result,
        Err(GeometryError::PolyRefusal {
            code: "poly.inset_too_large",
            element: Some(Element::Face(3))
        })
    ));
}

#[test]
fn g6_41_selection_mistakes_and_cancellation_are_refused() {
    let wall = grid(2.0, 2.0, 2, 2).unwrap();
    assert_eq!(
        code(&inset_each(&wall, &[], 0.1, &context())),
        "poly.selection_empty"
    );
    assert_eq!(
        code(&inset_each(&wall, &[1, 1], 0.1, &context())),
        "poly.selection_invalid"
    );
    assert_eq!(
        code(&extrude_each(&wall, &[99], [0.0, 0.0, 1.0], &context())),
        "poly.selection_invalid"
    );
    let too_many: Vec<usize> = (0..=MAX_INDIVIDUAL_FACES).collect();
    let big = grid(10.0, 10.0, 80, 52).unwrap();
    assert_eq!(
        code(&inset_each(&big, &too_many, 0.01, &context())),
        "poly.budget_exceeded"
    );
    let cancelled = context();
    cancelled.cancel();
    assert!(matches!(
        inset_each(&wall, &[0], 0.1, &cancelled),
        Err(GeometryError::Cancelled)
    ));
}
