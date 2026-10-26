//! Regression coverage for defaults, collection order, and author-facing problems.
#![expect(clippy::unwrap_used, reason = "test fixtures must parse and commit")]
use crate::{Evaluator, Fail, Quality};
use ve_graph::{CommandBatch, Outcome, Session, TransactionId, builtin_registry, parse_text};

fn evaluate(source: &str) -> Result<ve_game::GameDef, Fail> {
    let commands = parse_text(&builtin_registry(), source).unwrap().commands;
    let mut session = Session::with_builtin(9);
    let outcome = session.apply(&CommandBatch {
        doc: session.document().id.clone(),
        base_revision: 0,
        transaction: TransactionId::parse("t_rolling_regressions").unwrap(),
        label: None,
        commands,
    });
    assert!(matches!(outcome, Outcome::Committed { .. }), "{outcome:?}");
    Ok(Evaluator::default()
        .evaluate(session.document(), Quality::Final)?
        .game
        .unwrap())
}

#[test]
fn omitted_parameters_use_defaults_and_plain_kinds_have_no_rolling_keys() {
    let game = evaluate(
        "ball = game.kind@1(name=\"ball\")\n\
         ground = game.terrain@1(parts=[\"field\"])\n\
         felt = game.surface@1(name=\"felt\", parts=[\"table\"], decel=0.5)\n\
         all = game.collect@1(ball, ground, felt)\noutput rules = all\n",
    )
    .unwrap();
    assert_eq!(game.terrain().unwrap().cell, 0.25);
    assert_eq!(game.surfaces()[0].bounce, 0.5);
    let text = game.describe();
    let kind = text
        .lines()
        .find(|line| line.starts_with("kind ball "))
        .unwrap();
    for key in ["roll=", "swing=", "reach=", "loft=", "power=", "pocket="] {
        assert!(!kind.contains(key), "{kind}");
    }
}

#[test]
fn nested_collects_preserve_the_supplied_surface_order() {
    let game = evaluate(
        "ball = game.kind@1(name=\"ball\", roll=true)\n\
         a = game.surface@1(name=\"first\", parts=[\"table\"], decel=1)\n\
         z = game.surface@1(name=\"last\", parts=[\"table\"], decel=2)\n\
         nested = game.collect@1(z, a)\n\
         all = game.collect@1(ball, nested)\noutput rules = all\n",
    )
    .unwrap();
    assert_eq!(
        game.surfaces()
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        ["last", "first"]
    );
}

#[test]
fn a_second_terrain_names_the_authored_node() {
    let error = evaluate(
        "a = game.terrain@1(parts=[\"field\"])\n\
         second_ground = game.terrain@1(parts=[\"table\"])\n\
         all = game.collect@1(a, second_ground)\noutput rules = all\n",
    )
    .unwrap_err();
    assert_eq!(error.code, "game.terrain");
    assert!(error.message.contains("second_ground"), "{error:?}");
    assert!(error.node.is_some(), "{error:?}");
}

#[test]
fn zero_cell_zero_power_and_empty_part_names_name_the_node() {
    for (statement, label, key) in [
        (
            "bad_ground = game.terrain@1(parts=[\"field\"], cell=0)",
            "bad_ground",
            "cell",
        ),
        (
            "bad_tool = game.kind@1(name=\"tool\", swing=true, power=0)",
            "bad_tool",
            "power",
        ),
        (
            "bad_surface = game.surface@1(name=\"felt\", parts=[\"\"], decel=1)",
            "bad_surface",
            "parts",
        ),
    ] {
        let error = evaluate(&format!(
            "{statement}\nall = game.collect@1({label})\noutput rules = all\n"
        ))
        .unwrap_err();
        assert!(
            error.message.contains(label) && error.message.contains(key),
            "{error:?}"
        );
        assert!(error.node.is_some(), "{error:?}");
    }
}
