//! A sculpted `body.human` remains composable with character tools.
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "fixture setup unwraps its known pack and graph"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality, set_human_pack};

fn evaluate(source: &str) -> Evaluation {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/human/human.pack");
    set_human_pack(&std::fs::read(path).unwrap()).unwrap();
    let commands = parse_text(&builtin_registry(), source).unwrap().commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_sculpt_human").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap()
}

#[test]
fn sculpt_keeps_a_human_rig_for_skinning_eyes_and_hair() {
    let result = evaluate(
        "body = body.human@1()\n\
         edited = poly.sculpt@1(body, strokes=\"grab 0 0.9 0.1 0.2 0 0 0.015\", falloff=\"surface\")\n\
         root_n = body.bone@1(edited, bone=\"root\")\n\
         spine01_n = body.bone@1(edited, bone=\"spine01\")\n\
         spine03_n = body.bone@1(edited, bone=\"spine03\")\n\
         head_n = body.bone@1(edited, bone=\"head\")\n\
         skin = body.skin@1(edited, bones=\"root; spine01; spine03; head\")\n\
         eyes = body.eye@1(edited)\n\
         hair = body.hair@1(edited, density=0.1)\n\
         outfit = body.outfit@1(edited, asset=\"female_casualsuit01\", bones=\"root; spine03; spine01; head\")\n\
         output root = root_n\noutput spine01 = spine01_n\noutput spine03 = spine03_n\noutput head = head_n\n\
         output original_body = body\noutput edited_body = edited\noutput person = skin\n\
         output eyes = eyes\noutput hair = hair\noutput outfit = outfit\n",
    );
    let person = result
        .parts
        .iter()
        .find(|part| part.name == "person")
        .unwrap();
    let skin = person.skin.as_ref().expect("the edited person is skinned");
    assert_eq!(skin.joints.len(), person.positions.len());
    assert_eq!(skin.weights.len(), person.positions.len());
    assert!(result.parts.iter().any(|part| part.name == "eyes"));
    assert!(result.parts.iter().any(|part| part.name == "hair"));
    assert!(result.parts.iter().any(|part| part.name == "outfit"));
    let body_positions = |name: &str| {
        &result
            .parts
            .iter()
            .find(|part| part.name == name)
            .unwrap()
            .positions
    };
    let max_delta = body_positions("edited_body")
        .iter()
        .zip(body_positions("original_body"))
        .map(|(a, b)| {
            ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
        })
        .fold(0.0_f32, f32::max);
    assert!(
        max_delta > 0.001,
        "the sculpt moves the surface: {max_delta}"
    );
}
