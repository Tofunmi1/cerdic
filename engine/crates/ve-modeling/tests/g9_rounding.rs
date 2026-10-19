//! B6: the typed fillet profile and solid round operator reach modeling evaluation.
#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "fixture failures include the model diagnostic rather than hiding it"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

fn evaluate(source: &str) -> ve_modeling::Evaluation {
    let parsed = parse_text(&builtin_registry(), source).expect("rounding fixture parses");
    let mut session = Session::with_builtin(209);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g9_rounding").expect("transaction"),
        label: None,
        commands: parsed.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap_or_else(|error| panic!("{}: {}", error.code, error.message))
}

#[test]
fn solid_round_is_available_to_the_typed_graph_and_preserves_bounds() {
    let result = evaluate(
        "box = solid.box@1(size=[1, 1, 1])\n\
         softened = solid.round@1(box, radius=0.12, segments=10)\n\
         output softened",
    );
    assert_eq!(result.parts.len(), 1);
    let part = &result.parts[0];
    assert!(
        part.volume < 1.0 && part.volume > 0.9,
        "volume {}",
        part.volume
    );
    for axis in 0..3 {
        let (minimum, maximum) = part.positions.iter().fold(
            (f32::INFINITY, f32::NEG_INFINITY),
            |(minimum, maximum), point| (minimum.min(point[axis]), maximum.max(point[axis])),
        );
        assert!(((maximum - minimum) - 1.0).abs() < 1.0e-5);
    }
}

#[test]
fn profile_fillet_can_feed_an_extrusion() {
    let result = evaluate(
        "outline = profile.rect@1(size=[2, 2])\n\
         softened = profile.fillet@1(outline, radius=0.2)\n\
         prism = solid.extrude@1(softened, height=1)\n\
         output prism",
    );
    let expected = 4.0 - (4.0 - std::f64::consts::PI) * 0.04;
    assert!((result.parts[0].volume - expected).abs() < 0.002);
}

#[test]
fn round_operator_rejects_a_core_thinner_than_its_diameter() {
    let parsed = parse_text(
        &builtin_registry(),
        "box = solid.box@1(size=[0.2, 0.4, 0.6])\n\
         softened = solid.round@1(box, radius=0.1, segments=12)\n\
         output softened",
    )
    .expect("rounding refusal fixture parses");
    let mut session = Session::with_builtin(210);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g9_round_refusal").expect("transaction"),
        label: None,
        commands: parsed.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    let failure = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .expect_err("round consumes the thin box core");
    assert_eq!(failure.code, "kernel.invalid_input");
}

#[test]
fn solid_chamfer_is_available_to_the_typed_graph_and_keeps_the_bounding_box() {
    let result = evaluate(
        "box = solid.box@1(size=[1, 1, 1])\n\
         cut = solid.chamfer@1(box, width=0.12)\n\
         output cut",
    );
    assert_eq!(result.parts.len(), 1);
    let part = &result.parts[0];
    assert!(
        part.volume < 1.0 && part.volume > 0.8,
        "volume {}",
        part.volume
    );
    for axis in 0..3 {
        let (minimum, maximum) = part.positions.iter().fold(
            (f32::INFINITY, f32::NEG_INFINITY),
            |(minimum, maximum), point| (minimum.min(point[axis]), maximum.max(point[axis])),
        );
        assert!(((maximum - minimum) - 1.0).abs() < 1.0e-5);
    }
}

#[test]
fn chamfer_operator_rejects_a_core_thinner_than_twice_its_width() {
    let parsed = parse_text(
        &builtin_registry(),
        "box = solid.box@1(size=[0.2, 0.4, 0.6])\n\
         cut = solid.chamfer@1(box, width=0.1)\n\
         output cut",
    )
    .expect("chamfer refusal fixture parses");
    let mut session = Session::with_builtin(211);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g9_chamfer_refusal").expect("transaction"),
        label: None,
        commands: parsed.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    let failure = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .expect_err("chamfer consumes the thin box core");
    assert_eq!(failure.code, "kernel.invalid_input");
}

#[test]
fn pistol_and_study_room_fixtures_use_the_new_rounding_operators() {
    for source in [
        include_str!("gallery/visual/pistol_prop.txt"),
        include_str!("gallery/visual/study_room.txt"),
    ] {
        let result = evaluate(source);
        assert!(!result.parts.is_empty());
        assert!(result.parts.iter().all(|part| part.volume.is_finite()));
    }
}
