//! `field.tube` with `end_radius`: a limb, tail or horn that narrows along its length.
#![expect(
    clippy::unwrap_used,
    reason = "the helper unwraps fixtures that are known to be valid"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn volume(tube_params: &str) -> f64 {
    let source = format!(
        "tube = field.tube@1(points=[[0, 0, 0], [0, 0.6, 0.05], [0, 1.2, 0]], {tube_params})\n\
         body = field.to_solid@1(tube, min=[-0.5, -0.5, -0.5], max=[0.5, 1.7, 0.5], edge_length=0.02)\n\
         output body = body\n"
    );
    let commands = parse_text(&builtin_registry(), &source).unwrap().commands;
    let mut session = Session::with_builtin(3);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g11_tapered_tube").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap()
        .parts[0]
        .volume
}

#[test]
fn a_tapered_tube_lies_between_the_two_constant_tubes() {
    let thick = volume("radius=0.2");
    let thin = volume("radius=0.08");
    let tapered = volume("radius=0.2, end_radius=0.08");
    assert!(
        thin < tapered && tapered < thick,
        "{thin} < {tapered} < {thick}"
    );
    // Narrowing along the line is the average of the two ends to first order (a cone frustum's volume
    // is not the plain average, so the bounds are loose).
    let middle = volume("radius=0.14");
    assert!(
        (tapered - middle).abs() / middle < 0.2,
        "{tapered} against {middle}"
    );
}

#[test]
fn a_zero_end_radius_means_constant() {
    let plain = volume("radius=0.15");
    let explicit = volume("radius=0.15, end_radius=0");
    assert!(
        (plain - explicit).abs() < 1e-9,
        "{plain} against {explicit}"
    );
}

#[test]
fn the_taper_is_symmetric_in_direction() {
    let forward = volume("radius=0.2, end_radius=0.08");
    let source = "tube = field.tube@1(points=[[0, 1.2, 0], [0, 0.6, 0.05], [0, 0, 0]], radius=0.08, end_radius=0.2)\n\
         body = field.to_solid@1(tube, min=[-0.5, -0.5, -0.5], max=[0.5, 1.7, 0.5], edge_length=0.02)\n\
         output body = body\n";
    let commands = parse_text(&builtin_registry(), source).unwrap().commands;
    let mut session = Session::with_builtin(3);
    session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g11_tapered_reverse").unwrap(),
        label: None,
        commands,
    });
    let backward = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap()
        .parts[0]
        .volume;
    assert!(
        (forward - backward).abs() / forward < 0.02,
        "{forward} against {backward}"
    );
}
