//! The hair-cards macro (`kits/hair_cards.txt`): strips of cloth pinned along their roots to a bone, from one line. Do not weaken.
#![expect(clippy::unwrap_used, clippy::panic, reason = "fixture")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

const PRELUDE: &str = include_str!("../kits/hair_cards.txt");

fn evaluate(program: &str) -> Result<Evaluation, (String, String)> {
    let source = format!("{PRELUDE}\n{program}\n");
    let commands = parse_text(&builtin_registry(), &source)
        .map_err(|e| ("parse".to_owned(), format!("{e:?}")))?
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_hair").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .map_err(|e| (e.code.clone(), e.message.clone()))
}

const HEAD: &str = "box = solid.box@1(size=[0.2, 0.25, 0.2])\nhead_at = xform.move@1(box, by=[0, 1.7, 0])\nhead = rig.bone@1(head_at, head=[0, 1.6, 0], tail=[0, 1.8, 0])\noutput head = head\n";

#[test]
fn one_line_makes_strips_that_hang_from_the_head_bone() {
    let result = evaluate(&format!(
        "{HEAD}hair_cards(\"hair\", \"head\", 0, 1.78, 0, 0.1, 0.35, 28, 0.045, 3)"
    ))
    .unwrap_or_else(|e| panic!("{e:?}"));
    let part = result.parts.iter().find(|p| p.name == "hair").unwrap();
    let cloth = part.cloth.as_ref().expect("the hair is live cloth");
    assert_eq!(cloth.pin_bone, "head");
    assert_eq!(cloth.pins.len(), 28 * 2, "two root points per strip");
    assert_eq!(
        part.positions.len(),
        28 * 2 * 11,
        "ten segments below the root edge"
    );
    // Every root is on the ring at the top; the strips hang below it.
    for &pin in &cloth.pins {
        assert!((part.positions[pin as usize][1] - 1.78).abs() < 1e-4);
    }
    let lowest = part.positions.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
    assert!(
        lowest < 1.78 - 0.25 && lowest > 1.78 - 0.5,
        "strips are 0.28 to 0.42 m long: lowest {lowest}"
    );
    assert!(
        result
            .warnings
            .iter()
            .all(|w| !w.code.starts_with("cloth.")),
        "{:?}",
        result.warnings
    );
}

#[test]
fn the_same_seed_gives_the_same_hair_and_another_seed_another() {
    let make = |seed: u32| {
        let result = evaluate(&format!(
            "{HEAD}hair_cards(\"hair\", \"head\", 0, 1.78, 0, 0.1, 0.35, 12, 0.045, {seed})"
        ))
        .unwrap();
        result
            .parts
            .iter()
            .find(|p| p.name == "hair")
            .unwrap()
            .positions
            .clone()
    };
    assert_eq!(make(3), make(3));
    assert_ne!(make(3), make(4));
}

#[test]
fn a_pin_bone_that_is_missing_drops_the_cloth_with_a_warning() {
    let result =
        evaluate("hair_cards(\"hair\", \"nobody\", 0, 1.78, 0, 0.1, 0.35, 8, 0.045, 1)").unwrap();
    assert!(
        result
            .parts
            .iter()
            .find(|p| p.name == "hair")
            .unwrap()
            .cloth
            .is_none()
    );
    assert!(
        result.warnings.iter().any(|w| w.code == "cloth.pin_bone"),
        "{:?}",
        result.warnings
    );
}
