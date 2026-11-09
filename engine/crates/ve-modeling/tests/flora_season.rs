//! `flora.tree` seasons: summer is the default green, spring and autumn recolour the leaves, winter is bare, and a colour chosen by hand wins.
#![expect(clippy::unwrap_used, reason = "fixtures are known to be valid")]
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluation, Evaluator, Quality};

fn tree(params: &str) -> Evaluation {
    let source = format!("t = flora.tree@1(kind=\"oak\", height=6, seed=2, lod=2{params})\noutput t = t\n");
    let commands = parse_text(&builtin_registry(), &source).unwrap().commands;
    let mut session = Session::with_builtin(9);
    let applied = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_season").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(applied, Outcome::Committed { .. }), "{applied:?}");
    Evaluator::default().evaluate(session.document(), Quality::Final).unwrap()
}

fn leaf_program(result: &Evaluation) -> String {
    // the bark has a texture program too; the leaves are the one that draws leaves with `hash`
    result.parts.iter().filter_map(|p| p.material.as_ref()).filter_map(|m| m.texture_program.clone()).find(|p| p.contains("hash")).unwrap_or_default()
}

#[test]
fn a_winter_tree_is_bare_branches() {
    let summer = tree("");
    let winter = tree(", season=\"winter\"");
    assert!(summer.parts.len() > winter.parts.len(), "the leaf region is gone: {} against {}", summer.parts.len(), winter.parts.len());
    assert!(leaf_program(&winter).is_empty(), "no leaf material is left");
    let triangles = |r: &Evaluation| r.parts.iter().map(|p| p.indices.len() / 3).sum::<usize>();
    assert!(triangles(&winter) < triangles(&summer) / 2, "bark only: {} against {}", triangles(&winter), triangles(&summer));
}

#[test]
fn spring_and_autumn_recolour_the_leaves_and_summer_is_the_default() {
    let (summer, spring, autumn) = (leaf_program(&tree("")), leaf_program(&tree(", season=\"spring\"")), leaf_program(&tree(", season=\"autumn\"")));
    assert!(!summer.is_empty());
    assert_eq!(summer, leaf_program(&tree(", season=\"summer\"")), "summer is the default");
    assert_ne!(spring, summer);
    assert_ne!(autumn, summer);
    assert_ne!(autumn, spring);
}

#[test]
fn a_leaf_colour_chosen_by_hand_wins_over_the_season() {
    let by_hand = ", leaves=[0.5, 0.1, 0.6]";
    let plain = leaf_program(&tree(by_hand));
    assert_eq!(plain, leaf_program(&tree(&format!("{by_hand}, season=\"autumn\""))));
    assert!(plain.contains("0.500"), "the purple reaches the program");
}
