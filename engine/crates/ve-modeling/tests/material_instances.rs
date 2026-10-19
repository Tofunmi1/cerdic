//! A material on a placement set colours every placement and keeps the set shared.
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};
use ve_modeling::{Evaluator, Quality};

#[test]
fn a_material_on_an_instance_set_keeps_the_set_and_colours_each_placement() {
    let text = "post = solid.box@1(size=[0.05, 1, 0.05])\n\
                posts = inst.array@1(post, count_x=12, spacing=[0.2, 0, 0])\n\
                red = solid.material@1(posts, color=[0.9, 0.1, 0.1], roughness=0.3)\n\
                moved = xform.move@1(red, by=[0, 0.5, 0])\n\
                output fence = moved";
    let batch = parse_text(&builtin_registry(), text).unwrap_or_else(|e| panic!("{e:?}"));
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_material_set").unwrap_or_else(|e| panic!("{e:?}")),
        label: None,
        commands: batch.commands,
    });
    assert!(
        matches!(outcome, Outcome::Committed { .. }),
        "the text commits: {outcome:?}"
    );
    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap_or_else(|e| panic!("{}: {}", e.code, e.message));
    assert_eq!(result.parts.len(), 1);
    let part = &result.parts[0];
    assert_eq!(
        part.transforms.as_ref().map(Vec::len),
        Some(12),
        "still twelve placements of one prototype"
    );
    let material = part.material.as_ref().expect("the set carries its surface");
    assert!((material.color[0] - 0.9).abs() < 1.0e-6);
    assert!((material.roughness - 0.3).abs() < 1.0e-6);
}

#[test]
fn solid_material_accepts_the_typed_bounded_procedural_recipe_set() {
    use ve_modeling::MaterialRecipe;

    for (name, expected) in [
        ("wood", MaterialRecipe::Wood),
        ("planks", MaterialRecipe::Planks),
        ("brushed_metal", MaterialRecipe::BrushedMetal),
        ("leather", MaterialRecipe::Leather),
        ("fabric", MaterialRecipe::Fabric),
        ("brick", MaterialRecipe::Brick),
        ("concrete", MaterialRecipe::Concrete),
        ("marble", MaterialRecipe::Marble),
        ("painted_wall", MaterialRecipe::PaintedWall),
    ] {
        let source = format!(
            "box = solid.box@1(size=[1,1,1])\nsurface = solid.material@1(box, recipe=\"{name}\", texture_scale=2.5)\noutput surface"
        );
        let parsed = parse_text(&builtin_registry(), &source)
            .unwrap_or_else(|error| panic!("recipe {name} parses: {error:?}"));
        let mut session = Session::with_builtin(2);
        let outcome = session.apply(&CommandBatch {
            doc: session.document().id.clone(),
            base_revision: 0,
            transaction: TransactionId::parse("t_recipe").expect("fixture transaction"),
            label: None,
            commands: parsed.commands,
        });
        assert!(
            matches!(outcome, Outcome::Committed { .. }),
            "{name}: {outcome:?}"
        );
        let result = Evaluator::default()
            .evaluate(session.document(), Quality::Final)
            .unwrap_or_else(|error| panic!("{name}: {}", error.message));
        let material = result.parts[0]
            .material
            .as_ref()
            .expect("material recipe remains attached to the output");
        assert_eq!(material.recipe, expected, "{name}");
        assert!((material.texture_scale - 2.5).abs() < 1.0e-6, "{name}");
    }
}

#[test]
fn procedural_material_recipes_survive_save_load_undo_and_redo() {
    let source = "box = solid.box@1(size=[1,1,1])\nsurface = solid.material@1(box, recipe=\"brushed_metal\", texture_scale=3.25)\noutput surface";
    let parsed = parse_text(&builtin_registry(), source)
        .unwrap_or_else(|error| panic!("recipe fixture parses: {error:?}"));
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_recipe_history").expect("fixture transaction"),
        label: None,
        commands: parsed.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }));

    let saved = session.document().canonical_json();
    let loaded: ve_graph::Document =
        serde_json::from_str(&saved).unwrap_or_else(|error| panic!("save reloads: {error}"));
    assert_eq!(
        &loaded,
        session.document(),
        "reload preserves recipe parameters"
    );
    let after = session.document().clone();
    assert!(session.undo().is_some());
    assert!(
        session.document().nodes.is_empty(),
        "undo removes the recipe graph"
    );
    assert!(session.redo().is_some());
    assert!(
        session.document().same_content(&after),
        "redo restores the recipe graph"
    );

    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Final)
        .unwrap_or_else(|error| panic!("redo evaluates: {}", error.message));
    let material = result.parts[0]
        .material
        .as_ref()
        .expect("redo restores the procedural material");
    assert_eq!(material.recipe, ve_modeling::MaterialRecipe::BrushedMetal);
    assert!((material.texture_scale - 3.25).abs() < 1.0e-6);
}

#[test]
fn pistol_prop_slide_and_grip_keep_their_procedural_materials() {
    let parsed = parse_text(
        &builtin_registry(),
        include_str!("gallery/visual/pistol_prop.txt"),
    )
    .unwrap_or_else(|error| panic!("pistol fixture parses: {error:?}"));
    let mut session = Session::with_builtin(2);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_pistol_materials").expect("fixture transaction"),
        label: None,
        commands: parsed.commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }));
    let result = Evaluator::default()
        .evaluate(session.document(), Quality::Preview)
        .unwrap_or_else(|error| panic!("pistol evaluation: {}: {}", error.code, error.message));
    let recipe_for = |name: &str| {
        result
            .parts
            .iter()
            .find(|part| part.name == name)
            .and_then(|part| part.material.as_ref())
            .map_or_else(
                || panic!("pistol output {name} has its recipe material"),
                |material| material.recipe,
            )
    };
    assert_eq!(
        recipe_for("slide"),
        ve_modeling::MaterialRecipe::BrushedMetal
    );
    assert_eq!(recipe_for("frame"), ve_modeling::MaterialRecipe::Leather);
}
