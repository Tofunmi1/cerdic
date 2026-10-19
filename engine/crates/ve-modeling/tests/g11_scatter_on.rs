//! `inst.scatter_on`: placements drawn uniformly by area over a solid's surface, oriented to its normals.
//!
//! The surface is a unit cube (it spans -0.5 to 0.5 on every axis), so "on the surface" and "which face" are exact:
//! a point is on the face of the axis whose coordinate has magnitude 0.5.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    clippy::cast_precision_loss,
    reason = "fixture failures include the model diagnostic; counts are tiny"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn run(source: &str) -> Result<ve_modeling::Evaluation, String> {
    let parsed = parse_text(&builtin_registry(), source).map_err(|e| format!("{e:?}"))?;
    let mut session = Session::with_builtin(11);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g11_scatter_on").unwrap(),
        label: None,
        commands: parsed.commands,
    });
    if !matches!(outcome, Outcome::Committed { .. }) {
        return Err(format!("{outcome:?}"));
    }
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| {
            format!(
                "{}: {} | {}",
                e.code,
                e.message,
                e.suggestion.clone().unwrap_or_default()
            )
        })
}

fn program(extra: &str) -> String {
    format!(
        "hill = solid.box@1(size=[1, 1, 1])\n\
         pebble = solid.box@1(size=[0.02, 0.02, 0.02])\n\
         placed = inst.scatter_on@1(pebble, hill, {extra})\n\
         output placed"
    )
}

fn place(extra: &str) -> Vec<[f32; 16]> {
    run(&program(extra)).unwrap_or_else(|e| panic!("{e}")).parts[0]
        .transforms
        .clone()
        .unwrap()
}

fn at(m: &[f32; 16]) -> [f64; 3] {
    [f64::from(m[12]), f64::from(m[13]), f64::from(m[14])]
}

/// The unit normal of the cube face a point is on: the axis whose coordinate has magnitude 0.5.
fn face_normal(p: [f64; 3]) -> [f64; 3] {
    let axis = (0..3)
        .max_by(|&a, &b| p[a].abs().total_cmp(&p[b].abs()))
        .unwrap();
    let mut n = [0.0; 3];
    n[axis] = p[axis].signum();
    n
}

fn column_y(m: &[f32; 16]) -> [f64; 3] {
    [f64::from(m[4]), f64::from(m[5]), f64::from(m[6])]
}

fn length(v: [f64; 3]) -> f64 {
    v.iter().map(|c| c * c).sum::<f64>().sqrt()
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.iter().zip(&b).map(|(x, y)| x * y).sum()
}

#[test]
fn the_requested_number_of_copies_is_placed() {
    assert_eq!(place("count=200").len(), 200);
}

#[test]
fn every_copy_sits_on_the_surface() {
    for m in place("count=300") {
        let p = at(&m);
        let largest = p.iter().fold(0.0_f64, |a, c| a.max(c.abs()));
        assert!(
            (largest - 0.5).abs() < 1.0e-4,
            "point {p:?} is not on the cube"
        );
        assert!(
            p.iter().all(|c| c.abs() <= 0.5 + 1.0e-4),
            "point {p:?} is outside the cube"
        );
    }
}

#[test]
fn copies_are_spread_by_area_over_the_six_faces() {
    let mut share = std::collections::BTreeMap::new();
    let all = place("count=2000");
    for m in &all {
        let n = face_normal(at(m));
        *share.entry(format!("{n:?}")).or_insert(0_usize) += 1;
    }
    assert_eq!(share.len(), 6, "faces hit: {share:?}");
    for (face, count) in &share {
        let fraction = *count as f64 / all.len() as f64;
        assert!((0.14..=0.19).contains(&fraction), "{face}: {fraction}");
    }
}

#[test]
fn aligned_copies_stand_on_the_face_normal() {
    for m in place("count=300") {
        let (up, normal) = (column_y(&m), face_normal(at(&m)));
        for axis in 0..3 {
            assert!(
                (up[axis] - normal[axis]).abs() < 1.0e-4,
                "up {up:?} vs normal {normal:?}"
            );
        }
    }
}

#[test]
fn min_distance_is_kept_and_an_impossible_one_is_refused_with_a_fix() {
    let spaced = place("count=60, min_distance=0.2");
    assert_eq!(spaced.len(), 60);
    for (i, a) in spaced.iter().enumerate() {
        for b in &spaced[i + 1..] {
            let d = length([
                at(a)[0] - at(b)[0],
                at(a)[1] - at(b)[1],
                at(a)[2] - at(b)[2],
            ]);
            assert!(d >= 0.2 - 1.0e-4, "{d} apart");
        }
    }
    let error = run(&program("count=5000, min_distance=0.5")).unwrap_err();
    assert!(error.contains("inst.unsatisfiable"), "{error}");
    assert!(
        error.contains("count") && error.contains("min_distance"),
        "{error}"
    );
    let too_many = run(&program("count=100000")).unwrap_err();
    assert!(too_many.contains("param.range"), "{too_many}");
}

#[test]
fn region_keeps_copies_on_the_up_down_or_side_faces() {
    for m in place("count=200, region=\"up\"") {
        assert!((at(&m)[1] - 0.5).abs() < 1.0e-4, "{:?}", at(&m));
    }
    for m in place("count=200, region=\"down\"") {
        assert!((at(&m)[1] + 0.5).abs() < 1.0e-4, "{:?}", at(&m));
    }
    for m in place("count=200, region=\"side\"") {
        let p = at(&m);
        assert!(
            (p[0].abs().max(p[2].abs()) - 0.5).abs() < 1.0e-4,
            "{p:?} is on the top or bottom"
        );
    }
}

#[test]
fn the_seed_decides_the_layout() {
    let first = place("count=80, seed=7");
    assert_eq!(first, place("count=80, seed=7"));
    assert_ne!(first, place("count=80, seed=8"));
}

#[test]
fn scale_jitter_and_tilt_stay_inside_their_bounds() {
    let scales: Vec<f64> = place("count=400, scale_jitter=0.5")
        .iter()
        .map(|m| length(column_y(m)))
        .collect();
    assert!(
        scales
            .iter()
            .all(|s| (0.5 - 1.0e-4..=1.5 + 1.0e-4).contains(s)),
        "{scales:?}"
    );
    let (low, high) = scales
        .iter()
        .fold((f64::MAX, f64::MIN), |(l, h), s| (l.min(*s), h.max(*s)));
    assert!(high - low > 0.5, "scales barely vary: {low}..{high}");
    let angles: Vec<f64> = place("count=400, tilt=30deg")
        .iter()
        .map(|m| {
            dot(column_y(m), face_normal(at(m)))
                .clamp(-1.0, 1.0)
                .acos()
                .to_degrees()
        })
        .collect();
    assert!(
        angles.iter().all(|a| *a <= 30.0 + 1.0e-2),
        "largest lean {:?}",
        angles.iter().fold(0.0_f64, |a, b| a.max(*b))
    );
    assert!(
        angles.iter().any(|a| *a > 5.0),
        "no copy leans more than 5 degrees"
    );
}

#[test]
fn the_count_can_come_from_a_value_node() {
    // value sockets and scatter together: 50 copies, the number taken from a value node.
    let result = run("n = value.number@1(value=50)\n\
         hill = solid.box@1(size=[1, 1, 1])\n\
         pebble = solid.box@1(size=[0.02, 0.02, 0.02])\n\
         placed = inst.scatter_on@1(pebble, hill, count=n)\n\
         output placed")
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(result.parts[0].transforms.as_ref().unwrap().len(), 50);
}
