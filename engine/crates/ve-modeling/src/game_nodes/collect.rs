use super::{GameFragment, Item, Section, at_node, label};
use crate::{Fail, cache::Value};
use std::{collections::BTreeMap, rc::Rc};
use ve_game::GameDef;
use ve_graph::{Node, NodeId};

pub(super) fn collect(node: &Node, items: Vec<Item>, name: &str) -> Result<Value, Fail> {
    check_duplicates(&items)?;
    check_counts(node, &items)?;
    let (source, lines) = source_text(&items);
    let mut definition = parse_definition(node, &source, &lines)?;
    definition.name = name.to_owned();
    let problem = validate_definition(node, &definition, &lines).err();
    let mut value = Value::nothing();
    value.game = Some(Rc::new(GameFragment {
        items,
        definition: Some(Rc::new(definition)),
        problem,
    }));
    Ok(value)
}

fn check_duplicates(items: &[Item]) -> Result<(), Fail> {
    let mut seen: BTreeMap<(Section, String), (NodeId, String)> = BTreeMap::new();
    for entry in items {
        if let Some(name) = &entry.name {
            let key = (entry.section, name.clone());
            if let Some(previous) = seen.insert(key, (entry.node.clone(), entry.label.clone())) {
                return Err(Box::new(
                    ve_graph::Diagnostic::error(
                        "game.duplicate",
                        format!(
                            "{} and {} declare the repeated game name {name:?}",
                            previous.1, entry.label
                        ),
                    )
                    .at_node(&entry.node)
                    .with_related(vec![previous.0]),
                ));
            }
        }
    }
    Ok(())
}

fn check_counts(node: &Node, items: &[Item]) -> Result<(), Fail> {
    let kinds = items
        .iter()
        .filter(|item| item.section == Section::Kind)
        .count();
    let rules = items
        .iter()
        .filter(|item| item.section == Section::Rule)
        .count();
    if kinds > 64 {
        return Err(at_node("budget.kinds", node, "game has more than 64 kinds"));
    }
    if rules > 256 {
        return Err(at_node(
            "budget.rules",
            node,
            "game has more than 256 rules",
        ));
    }
    let terrains: Vec<&Item> = items
        .iter()
        .filter(|item| item.section == Section::Terrain)
        .collect();
    if terrains.len() > 1 {
        let current = terrains[1];
        return Err(Box::new(
            ve_graph::Diagnostic::error(
                "game.terrain",
                format!(
                    "{}: only one terrain node is allowed per document",
                    current.label
                ),
            )
            .at_node(&current.node),
        ));
    }
    Ok(())
}

fn source_text(items: &[Item]) -> (String, Vec<&Item>) {
    let mut text = String::new();
    let mut lines = Vec::new();
    for section in [
        Section::Var,
        Section::State,
        Section::Level,
        Section::Kind,
        Section::Agent,
        Section::Controller,
        Section::Camera,
        Section::Health,
        Section::Projectile,
        Section::Weapon,
        Section::Brain,
        Section::Targeting,
        Section::Ability,
        Section::Nav,
        Section::Ground,
        Section::Terrain,
        Section::Surface,
        Section::Spawn,
        Section::Input,
        Section::Ui,
        Section::Rule,
        Section::Script,
        Section::Goal,
        Section::Hud,
    ] {
        for entry in items.iter().filter(|entry| entry.section == section) {
            text.push_str(&entry.text);
            text.push('\n');
            lines.push(entry);
        }
    }
    (text, lines)
}

fn parse_definition(node: &Node, text: &str, lines: &[&Item]) -> Result<GameDef, Fail> {
    // a group may name states, kinds and weapons that another group declares, so the names are looked up only in `validate_definition`
    GameDef::parse_partial(text).map_err(|errors| {
        let error = &errors[0];
        let entry = lines.get(error.line.saturating_sub(1));
        let node_id = entry.map_or_else(|| node.id.clone(), |item| item.node.clone());
        let shown = entry.map_or_else(|| label(node), |item| item.label.clone());
        let code = entry.map_or("game.validate", |item| match item.section {
            Section::Var => "game.var",
            Section::State => "game.state",
            Section::Level => "game.level",
            Section::Kind => "game.kind",
            Section::Agent => "game.agent",
            Section::Controller => "game.controller",
            Section::Camera => "game.camera",
            Section::Nav => "game.nav",
            Section::Ground => "game.ground",
            Section::Terrain => "game.terrain",
            Section::Surface => "game.surface",
            Section::Spawn => "game.spawn",
            Section::Input => "game.input",
            Section::Ui => "game.ui",
            Section::Health => "game.health",
            Section::Projectile => "game.projectile",
            Section::Weapon => "game.weapon",
            Section::Brain => "game.brain",
            Section::Targeting => "game.targeting",
            Section::Ability => "game.ability",
            Section::Rule => "game.rule",
            Section::Goal => "game.goal",
            Section::Hud => "game.hud",
            Section::Script => "game.script",
        });
        Box::new(
            ve_graph::Diagnostic::error(code, format!("{shown}: {}", error.message))
                .at_node(&node_id)
                .suggest(error.suggestion.clone()),
        )
    })
}

fn validate_definition(node: &Node, definition: &GameDef, lines: &[&Item]) -> Result<(), Fail> {
    let diagnostics = definition.validate();
    let Some(error) = diagnostics.first() else {
        return Ok(());
    };
    let entry = if error.message.starts_with("initial entity count") {
        lines
            .iter()
            .find(|item| item.section == Section::Spawn)
            .copied()
    } else if error.message.starts_with("goal variable") {
        lines
            .iter()
            .find(|item| item.section == Section::Goal)
            .copied()
    } else {
        lines.get(error.line.saturating_sub(1)).copied()
    };
    let node_id = entry.map_or_else(|| node.id.clone(), |item| item.node.clone());
    let shown = entry.map_or_else(|| label(node), |item| item.label.clone());
    let code = if error.message.contains("exceeds 2048") {
        "budget.entities"
    } else {
        "game.validate"
    };
    Err(Box::new(
        ve_graph::Diagnostic::error(code, format!("{shown}: {}", error.message))
            .at_node(&node_id)
            .suggest(error.suggestion.clone()),
    ))
}
