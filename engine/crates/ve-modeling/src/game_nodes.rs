//! `game.*` node fragments and `game.collect` validation (ADR 0062).
use crate::{Fail, cache::Value, fail};
use std::rc::Rc;
use ve_game::GameDef;
use ve_graph::{Node, NodeId, Value as Param};
mod agents;
mod collect;
mod combat;
mod hud;
mod rolling;
use hud::make_hud;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Section {
    Var,
    State,
    Level,
    Ui,
    Health,
    Projectile,
    Weapon,
    Brain,
    Targeting,
    Ability,
    Kind,
    Agent,
    Controller,
    Camera,
    Nav,
    Ground,
    Terrain,
    Surface,
    Spawn,
    Input,
    Rule,
    Script,
    Goal,
    Hud,
}

#[derive(Clone, Debug)]
struct Item {
    section: Section,
    node: NodeId,
    /// What the author called the node (its label, else its id): diagnostics speak in these.
    label: String,
    text: String,
    name: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct GameFragment {
    items: Vec<Item>,
    pub(crate) definition: Option<Rc<GameDef>>,
    /// Why a collected definition is not playable yet. Kits are joined before they are complete, so this is reported only for the game
    /// that reaches a scene output (see `finish`).
    problem: Option<Fail>,
}

impl GameFragment {
    /// The definition of a game that is a scene output, or the problem that makes it unplayable.
    pub(crate) fn finish(&self) -> Result<Option<GameDef>, Fail> {
        match (&self.problem, &self.definition) {
            (Some(problem), _) => Err(problem.clone()),
            (None, definition) => Ok(definition.as_deref().cloned()),
        }
    }

    pub(crate) fn estimated_bytes(&self) -> usize {
        self.items
            .iter()
            .map(|item| {
                item.node.to_string().capacity()
                    + item.text.capacity()
                    + item.name.as_ref().map_or(0, String::capacity)
                    + std::mem::size_of::<Item>()
            })
            .sum::<usize>()
            + self
                .definition
                .as_ref()
                .map_or(0, |definition| definition.describe().len())
    }
}

fn text<'a>(node: &'a Node, key: &str) -> Option<&'a str> {
    match node.params.get(key) {
        Some(Param::Str(value)) => Some(value),
        _ => None,
    }
}
fn number(node: &Node, key: &str) -> Result<f64, Fail> {
    node.params
        .get(key)
        .and_then(Param::as_f64)
        .ok_or_else(|| fail("game.value", format!("{} needs numeric {key}", node.id)))
}
fn number_or(node: &Node, key: &str, default: f64) -> f64 {
    node.params
        .get(key)
        .and_then(Param::as_f64)
        .unwrap_or(default)
}
fn label(node: &Node) -> String {
    node.label.clone().unwrap_or_else(|| node.id.to_string())
}
fn compact(value: &str) -> String {
    value.split_whitespace().collect()
}
fn item(node: &Node, section: Section, text: String, name: Option<String>) -> GameFragment {
    GameFragment {
        items: vec![Item {
            section,
            node: node.id.clone(),
            label: label(node),
            text,
            name,
        }],
        definition: None,
        problem: None,
    }
}

pub(crate) fn run(node: &Node, inputs: &[Rc<Value>]) -> Option<Result<Value, Fail>> {
    let family = node.op.family().as_str();
    if !family.starts_with("game.") {
        return None;
    }
    Some(build(node, inputs))
}

fn build(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let family = node.op.family().as_str();
    if family == "game.collect" {
        let items = inputs
            .iter()
            .filter_map(|value| value.game.as_deref())
            .flat_map(|fragment| fragment.items.iter().cloned())
            .collect();
        return collect::collect(node, items, text(node, "name").unwrap_or("game"));
    }
    let fragment = match family {
        "game.var" => make_var(node)?,
        "game.kind" => make_kind(node)?,
        "game.terrain" => rolling::terrain(node)?,
        "game.surface" => rolling::surface(node)?,
        "game.spawn" => make_spawn(node, inputs)?,
        "game.input" => make_input(node)?,
        "game.rule" => make_rule(node),
        "game.script" => make_script(node)?,
        "game.goal" => make_goal(node),
        "game.hud" => make_hud(node),
        "game.agent" => agents::agent(node)?,
        "game.controller" => agents::controller(node)?,
        "game.camera" => agents::camera(node)?,
        "game.ui" => agents::ui(node)?,
        "game.health" => combat::health(node)?,
        "game.projectile" => combat::projectile(node)?,
        "game.weapon" => combat::weapon(node)?,
        "game.brain" => combat::brain(node)?,
        "game.targeting" => combat::targeting(node)?,
        "game.ability" => combat::ability(node)?,
        "game.state" => agents::state(node)?,
        "game.level" => agents::level(node)?,
        "game.nav" => agents::nav(node),
        "game.ground" => item(
            node,
            Section::Ground,
            format!("ground y={}", number_or(node, "y", 0.0)),
            None,
        ),
        _ => return Err(at_node("game.node", node, "unknown game node")),
    };
    let mut value = Value::nothing();
    value.game = Some(Rc::new(fragment));
    Ok(value)
}

fn make_var(node: &Node) -> Result<GameFragment, Fail> {
    let name =
        text(node, "name").ok_or_else(|| fail("game.var", format!("{} needs name", node.id)))?;
    Ok(item(
        node,
        Section::Var,
        format!("var {name}={}", number(node, "value")?),
        Some(name.to_owned()),
    ))
}

fn make_kind(node: &Node) -> Result<GameFragment, Fail> {
    let name = text(node, "name").unwrap_or("");
    let tags = match node.params.get("tags") {
        Some(Param::List(values)) => values
            .iter()
            .filter_map(|value| match value {
                Param::Str(tag) => Some(tag.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(","),
        _ => String::new(),
    };
    let line = format!(
        "kind {name} size={} speed={}",
        number_or(node, "size", 0.5),
        number_or(node, "speed", 0.0)
    );
    let line = if tags.is_empty() {
        line
    } else {
        format!("{line} tags={tags}")
    };
    let part = text(node, "part").unwrap_or("");
    let line = if part.is_empty() {
        line
    } else {
        format!("{line} part={part}")
    };
    let solid: Vec<f64> = match node.params.get("solid") {
        Some(Param::List(values)) => values.iter().filter_map(Param::as_f64).collect(),
        _ => Vec::new(),
    };
    let line = if solid.len() == 3 && solid.iter().all(|extent| *extent > 0.0) {
        format!("{line} solid={},{},{}", solid[0], solid[1], solid[2])
    } else {
        line
    };
    let line = if matches!(node.params.get("grab"), Some(Param::Bool(true))) {
        format!("{line} grab=1")
    } else {
        line
    };
    let line = if matches!(node.params.get("fall"), Some(Param::Bool(true))) {
        format!("{line} fall=1 bounce={}", number_or(node, "bounce", 0.3))
    } else {
        format!("{line} bounce={}", number_or(node, "bounce", 0.3))
    };
    let line = rolling::kind_fields(node, line)?;
    Ok(item(node, Section::Kind, line, Some(name.to_owned())))
}

/// One entity per placement of a linked set, standing where the placement is.
fn spawn_from_set(node: &Node, set: &crate::instances::InstanceSet) -> GameFragment {
    let kind = text(node, "kind").unwrap_or("");
    let items = set
        .transforms
        .iter()
        .map(|m| Item {
            section: Section::Spawn,
            node: node.id.clone(),
            label: label(node),
            text: format!(
                "spawn {kind} at={},{},{}{}",
                m.w_axis.x,
                m.w_axis.y,
                m.w_axis.z,
                text(node, "level").filter(|level| !level.is_empty()).map(|level| format!(" level={level}")).unwrap_or_default()
            ),
            name: None,
        })
        .collect();
    GameFragment {
        items,
        definition: None,
        problem: None,
    }
}

fn make_spawn(node: &Node, inputs: &[Rc<Value>]) -> Result<GameFragment, Fail> {
    if let Some(set) = inputs.first().and_then(|value| value.instances.as_ref()) {
        return Ok(spawn_from_set(node, set));
    }
    let kind = text(node, "kind").unwrap_or("");
    let at = match node.params.get("at") {
        Some(Param::List(values)) if values.len() == 3 => {
            values.iter().filter_map(Param::as_f64).collect::<Vec<_>>()
        }
        _ => Vec::new(),
    };
    if at.len() != 3 {
        return Err(fail(
            "game.spawn",
            format!("{} needs a three-number at position", node.id),
        ));
    }
    let mut line = format!(
        "spawn {kind} at={},{},{} count={} spread={}",
        at[0],
        at[1],
        at[2],
        number_or(node, "count", 1.0),
        number_or(node, "spread", 0.0)
    );
    // Which way it faces to begin with, in degrees; left out when the node does not give one.
    if node.params.contains_key("heading") {
        line.push_str(&format!(" heading={}", number_or(node, "heading", 0.0)));
    }
    if let Some(level) = text(node, "level").filter(|level| !level.is_empty()) {
        line.push_str(&format!(" level={level}"));
    }
    Ok(item(node, Section::Spawn, line, None))
}

fn make_input(node: &Node) -> Result<GameFragment, Fail> {
    let name = text(node, "name").unwrap_or("");
    let axis = text(node, "axis").unwrap_or("");
    let key = text(node, "key").unwrap_or("");
    let target = text(node, "target").unwrap_or("");
    let follow = text(node, "follow").unwrap_or("");
    if !axis.is_empty() && !key.is_empty() {
        return Err(at_node(
            "game.input",
            node,
            "an input may declare an axis or a key, not both",
        ));
    }
    let mut line = format!("input {name}");
    if !axis.is_empty() {
        line.push_str(&format!(" axis={axis}"));
    }
    if !key.is_empty() {
        line.push_str(&format!(" key={key}"));
    }
    if !target.is_empty() {
        line.push_str(&format!(" target={target}"));
    }
    if !follow.is_empty() {
        line.push_str(&format!(" follow={follow}"));
    }
    for key in ["keys", "pad"] {
        if let Some(value) = text(node, key).filter(|value| !value.trim().is_empty()) {
            line.push_str(&format!(" {key}={}", compact(value)));
        }
    }
    if let Some(label) = text(node, "label").filter(|value| !value.trim().is_empty()) {
        line.push_str(&format!(" label={}", label.trim().replace(' ', "_")));
    }
    Ok(item(node, Section::Input, line, None))
}

fn make_rule(node: &Node) -> GameFragment {
    let mut line = format!("rule on={}", text(node, "on").unwrap_or(""));
    for key in ["a", "b", "when"] {
        if let Some(value) = text(node, key).filter(|value| !value.is_empty()) {
            let value = if key == "when" {
                compact(value)
            } else {
                value.to_owned()
            };
            line.push_str(&format!(" {key}={value}"));
        }
    }
    let every = number_or(node, "every", 0.0);
    if every > 0.0 {
        line.push_str(&format!(" every={every}"));
    }
    if let Some(state) = text(node, "state").filter(|state| !state.is_empty()) {
        line.push_str(&format!(" state={state}"));
    }
    line.push_str(&format!(" do={}", compact(text(node, "do").unwrap_or(""))));
    item(node, Section::Rule, line, None)
}

fn make_script(node: &Node) -> Result<GameFragment, Fail> {
    let source = text(node, "source").unwrap_or("");
    let library = text(node, "lib").unwrap_or("");
    let name = text(node, "name")
        .filter(|name| !name.is_empty())
        .map_or_else(
            || {
                if library.is_empty() {
                    label(node)
                } else {
                    library.to_owned()
                }
            },
            str::to_owned,
        );
    let script = if library.is_empty() {
        if source.trim().is_empty() {
            return Err(at_node(
                "game.script",
                node,
                "a script needs source (or lib)",
            ));
        }
        ve_game::ScriptDef::new(&name, source)
    } else {
        ve_game::ScriptDef::library(&name, library)
            .ok_or_else(|| at_node("game.script", node, "unknown library"))?
    };
    Ok(item(node, Section::Script, script.to_text(), Some(name)))
}

fn make_goal(node: &Node) -> GameFragment {
    let when = compact(text(node, "when").unwrap_or(""));
    item(
        node,
        Section::Goal,
        format!("goal {} when={}", text(node, "outcome").unwrap_or(""), when),
        None,
    )
}

fn at_node(code: &str, node: &Node, message: &str) -> Fail {
    Box::new(
        ve_graph::Diagnostic::error(code, format!("{}: {message}", label(node))).at_node(&node.id),
    )
}
