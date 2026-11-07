//! The face expression presets (`kits/face_presets.txt`): every one is accepted by `body.human` and visibly moves the face. Do not weaken.
#![expect(clippy::unwrap_used, clippy::panic, reason = "fixture")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

const PRELUDE: &str = include_str!("../kits/face_presets.txt");

fn head_points(face: &str) -> Vec<[f32; 3]> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/human/human.pack");
    ve_modeling::set_human_pack(&std::fs::read(path).unwrap()).unwrap();
    let text = format!("{PRELUDE}\nbody = body.human@1({face})\noutput person = body\n");
    let commands = parse_text(&builtin_registry(), &text)
        .unwrap_or_else(|e| panic!("{e:?}"))
        .commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_face").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap_or_else(|e| panic!("{face}: {} {}", e.code, e.message));
    assert!(
        result
            .warnings
            .iter()
            .all(|w| !w.code.starts_with("human.")),
        "{face}: {:?}",
        result.warnings
    );
    result
        .parts
        .iter()
        .find(|p| p.name == "person")
        .unwrap()
        .positions
        .iter()
        .copied()
        .filter(|p| p[1] > 1.4)
        .collect()
}

fn moved(neutral: &[[f32; 3]], other: &[[f32; 3]]) -> f32 {
    assert_eq!(neutral.len(), other.len(), "the same head vertices");
    neutral
        .iter()
        .zip(other)
        .map(|(a, b)| {
            ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
        })
        .fold(0.0, f32::max)
}

#[test]
fn every_preset_is_accepted_and_moves_the_face_by_at_least_a_millimetre() {
    let neutral = head_points("");
    for name in [
        "happy",
        "sad",
        "angry",
        "surprised",
        "wink",
        "blink",
        "talk",
        "disgust",
    ] {
        let face = head_points(&format!("face=\"${{face_{name}}}\""));
        let shift = moved(&neutral, &face);
        assert!(
            shift > 0.001,
            "face_{name} moves the face by only {shift} m"
        );
        assert!(
            shift < 0.1,
            "face_{name} moves a vertex {shift} m, more than a face can"
        );
    }
}

#[test]
fn expressions_differ_from_each_other_and_a_wink_is_one_sided() {
    let happy = head_points("face=\"${face_happy}\"");
    let sad = head_points("face=\"${face_sad}\"");
    assert!(
        moved(&happy, &sad) > 0.003,
        "happy and sad are different faces"
    );
    let neutral = head_points("");
    let wink = head_points("face=\"${face_wink}\"");
    let left: f32 = neutral
        .iter()
        .zip(&wink)
        .filter(|(a, _)| a[0] > 0.01)
        .map(|(a, b)| (a[1] - b[1]).abs() + (a[2] - b[2]).abs())
        .sum();
    let right: f32 = neutral
        .iter()
        .zip(&wink)
        .filter(|(a, _)| a[0] < -0.01)
        .map(|(a, b)| (a[1] - b[1]).abs() + (a[2] - b[2]).abs())
        .sum();
    assert!(
        left > right * 1.3,
        "the wink closes the left eye (left {left}, right {right})"
    );
}
