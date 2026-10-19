//! `solid.texture size`: how big the viewer bakes a surface's maps, carried on the material.
#![expect(
    clippy::panic,
    clippy::unwrap_used,
    reason = "fixture setup reports actionable failures"
)]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Material, Quality};

const PROGRAM: &str = "n = noise(scale=4); albedo = n";

fn apply(source: &str) -> (Session, Outcome) {
    let commands = parse_text(&builtin_registry(), source)
        .unwrap_or_else(|error| panic!("fixture did not parse: {error:?}"))
        .commands;
    let mut session = Session::with_builtin(77);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_g12_texture_size").unwrap(),
        label: None,
        commands,
    });
    (session, applied)
}

fn evaluate(source: &str) -> Evaluation {
    let (session, applied) = apply(source);
    assert!(
        matches!(applied, Outcome::Committed { .. }),
        "fixture refused: {applied:?}"
    );
    Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap_or_else(|error| panic!("fixture did not evaluate: {error:?}"))
}

fn material_of(size_param: &str) -> Material {
    let source = format!(
        "b = solid.box@1(size=[1, 1, 1])\nt = solid.texture@1(b, program=\"{PROGRAM}\"{size_param})\noutput t = t"
    );
    let result = evaluate(&source);
    result.parts[0].material.clone().expect("a textured part")
}

#[test]
fn size_is_auto_unless_asked_for() {
    assert_eq!(material_of("").texture_size, 0);
    assert_eq!(material_of(", size=\"auto\"").texture_size, 0);
}

#[test]
fn the_three_sizes_reach_the_material() {
    for side in [256, 512, 1024] {
        assert_eq!(
            material_of(&format!(", size=\"{side}\"")).texture_size,
            side
        );
    }
}

#[test]
fn other_sizes_are_refused_with_the_choices_named() {
    for bad in ["300", "2048", "0", "huge"] {
        let source = format!(
            "b = solid.box@1(size=[1, 1, 1])\nt = solid.texture@1(b, program=\"{PROGRAM}\", size=\"{bad}\")\noutput t = t"
        );
        let refused = match parse_text(&builtin_registry(), &source) {
            Err(_) => true,
            Ok(_) => !matches!(apply(&source).1, Outcome::Committed { .. }),
        };
        assert!(refused, "size {bad} should be refused");
    }
}

#[test]
fn a_later_node_keeps_the_size_and_old_results_without_it_still_load() {
    let source = format!(
        "b = solid.box@1(size=[1, 1, 1])\nt = solid.texture@1(b, program=\"{PROGRAM}\", size=\"256\")\nr = xform.move@1(t, by=[0, 1, 0])\noutput r = r"
    );
    let size = evaluate(&source).parts[0]
        .material
        .as_ref()
        .map(|m| m.texture_size);
    assert_eq!(size, Some(256));
    let mut json = serde_json::to_value(material_of(", size=\"512\"")).unwrap();
    assert!(
        json.as_object_mut()
            .unwrap()
            .remove("texture_size")
            .is_some()
    );
    let old: Material = serde_json::from_value(json).unwrap();
    assert_eq!(old.texture_size, 0);
}
