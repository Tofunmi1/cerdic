//! `inst.along_curve` (copies spaced by length along a curve, turned to follow it) and `inst.array` with relative spacing. Written before
//! the body; do not weaken.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "fixture setup unwraps data that is known to be valid"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn evaluate(source: &str) -> Result<Evaluation, (String, String)> {
    let commands = parse_text(&builtin_registry(), source)
        .unwrap_or_else(|e| panic!("{e:?}"))
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_arrays").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message.clone()))
}

/// Every copy's matrix, column-major.
fn copies(result: &Evaluation) -> Vec<[f32; 16]> {
    result
        .parts
        .iter()
        .flat_map(|part| part.transforms.iter().flatten().copied())
        .collect()
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

fn at(m: &[f32; 16]) -> [f32; 3] {
    [m[12], m[13], m[14]]
}

const POST: &str = "post = solid.box@1(size=[0.2, 0.2, 0.2])\n";

fn along(points: &str, closed: bool, count: u32, align: bool) -> Evaluation {
    evaluate(&format!(
        "{POST}c = curve.polyline@1(points={points}, closed={closed})\nrow = inst.along_curve@1(post, c, count={count}, align={align})\noutput o = row\n"
    ))
    .unwrap()
}

#[test]
fn copies_are_spaced_evenly_along_an_open_curve_with_one_at_each_end() {
    let result = along("[[0, 0, 0], [4, 0, 0]]", false, 5, true);
    let found = copies(&result);
    assert_eq!(found.len(), 5);
    for (k, m) in found.iter().enumerate() {
        assert!(near(at(m)[0], k as f32), "copy {k} at {:?}", at(m));
    }
}

#[test]
fn spacing_is_by_length_around_a_corner() {
    // An L: 3 m along X then 3 m along Z, seven copies one metre apart.
    let result = along("[[0, 0, 0], [3, 0, 0], [3, 0, 3]]", false, 7, true);
    let found = copies(&result);
    let expected = [
        [0.0, 0.0],
        [1.0, 0.0],
        [2.0, 0.0],
        [3.0, 0.0],
        [3.0, 1.0],
        [3.0, 2.0],
        [3.0, 3.0],
    ];
    for (m, want) in found.iter().zip(expected) {
        assert!(
            near(at(m)[0], want[0]) && near(at(m)[2], want[1]),
            "{:?} vs {want:?}",
            at(m)
        );
    }
}

#[test]
fn a_closed_curve_spaces_copies_all_the_way_round_without_repeating_the_start() {
    let result = along(
        "[[0, 0, 0], [2, 0, 0], [2, 0, 2], [0, 0, 2]]",
        true,
        4,
        false,
    );
    let found = copies(&result);
    let corners = [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]];
    assert_eq!(found.len(), 4);
    for (m, want) in found.iter().zip(corners) {
        assert!(
            near(at(m)[0], want[0]) && near(at(m)[2], want[1]),
            "{:?} vs {want:?}",
            at(m)
        );
    }
}

#[test]
fn an_aligned_copy_points_its_x_axis_along_the_curve_and_keeps_y_up() {
    let result = along("[[0, 0, 0], [0, 0, 4]]", false, 3, true);
    for m in copies(&result) {
        assert!(
            near(m[0], 0.0) && near(m[1], 0.0) && near(m[2], 1.0),
            "x axis along +Z: {:?}",
            [m[0], m[1], m[2]]
        );
        assert!(
            near(m[4], 0.0) && near(m[5], 1.0) && near(m[6], 0.0),
            "y stays up: {:?}",
            [m[4], m[5], m[6]]
        );
    }
}

#[test]
fn an_unaligned_copy_only_moves() {
    let result = along("[[0, 0, 0], [0, 0, 4]]", false, 3, false);
    for m in copies(&result) {
        assert!(
            near(m[0], 1.0) && near(m[5], 1.0) && near(m[10], 1.0),
            "orientation untouched"
        );
    }
}

#[test]
fn relative_spacing_counts_the_shapes_own_size() {
    let source = |relative: bool| {
        format!(
            "b = solid.box@1(size=[2, 1, 1])\nrow = inst.array@1(b, count_x=3, spacing=[1.5, 0, 0], relative={relative})\noutput o = row\n"
        )
    };
    let absolute = copies(&evaluate(&source(false)).unwrap());
    let relative = copies(&evaluate(&source(true)).unwrap());
    let xs = |set: &[[f32; 16]]| set.iter().map(|m| at(m)[0]).collect::<Vec<_>>();
    assert!(
        xs(&absolute)
            .iter()
            .zip([0.0, 1.5, 3.0])
            .all(|(a, b)| near(*a, b)),
        "{:?}",
        xs(&absolute)
    );
    assert!(
        xs(&relative)
            .iter()
            .zip([0.0, 3.0, 6.0])
            .all(|(a, b)| near(*a, b)),
        "1.5 widths of a 2 m box is 3 m: {:?}",
        xs(&relative)
    );
}

#[test]
fn relative_spacing_follows_the_shape_when_it_changes_size() {
    let source = "b = solid.box@1(size=[0.5, 1, 1])\nrow = inst.array@1(b, count_x=3, spacing=[2, 0, 0], relative=true)\noutput o = row\n";
    let xs: Vec<f32> = copies(&evaluate(source).unwrap())
        .iter()
        .map(|m| at(m)[0])
        .collect();
    assert!(
        xs.iter().zip([0.0, 1.0, 2.0]).all(|(a, b)| near(*a, b)),
        "two widths of a half-metre box: {xs:?}"
    );
}
