//! Game flow declarations: `state NAME [frozen=1]` (the first declared one is where the game starts; a frozen state stops the simulation:
//! a title, a pause, a game over), `level NAME [start=x,y,z]` (the first declared one is loaded at the start; spawns with `level=NAME`
//! belong to it), and the checks that every rule, action and spawn names a state or level that exists.
use std::collections::BTreeMap;

use crate::{Diagnostic, GameDef};

/// A named state of the game's flow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateDef {
    /// The name rules use (`go:NAME`, `state=NAME`, the event `state:NAME`).
    pub name: String,
    /// Whether the simulation stands still while the game is in this state.
    pub frozen: bool,
}

/// A level: spawns tagged with its name, and where the hero starts.
#[derive(Clone, Debug, PartialEq)]
pub struct LevelDef {
    /// The name `level:NAME` loads.
    pub name: String,
    /// Where entities tagged `hero` are put when the level loads.
    pub start: Option<[f64; 3]>,
}

fn flag(fields: &BTreeMap<String, String>, key: &str) -> Result<bool, String> {
    match fields.get(key).map(String::as_str) {
        None | Some("0" | "false") => Ok(false),
        Some("1" | "true") => Ok(true),
        Some(_) => Err(format!("{key} must be 1 or 0")),
    }
}

pub(super) fn parse_state(def: &mut GameDef, name: Option<&str>, fields: &BTreeMap<String, String>) -> Result<(), String> {
    let name = name.ok_or("a state needs a name")?;
    if def.states.iter().any(|state| state.name == name) {
        return Err(format!("duplicate state '{name}'"));
    }
    def.states.push(StateDef { name: name.to_owned(), frozen: flag(fields, "frozen")? });
    Ok(())
}

pub(super) fn parse_level(def: &mut GameDef, name: Option<&str>, fields: &BTreeMap<String, String>) -> Result<(), String> {
    let name = name.ok_or("a level needs a name")?;
    if def.levels.iter().any(|level| level.name == name) {
        return Err(format!("duplicate level '{name}'"));
    }
    let start = match fields.get("start") {
        None => None,
        Some(text) => {
            let numbers: Result<Vec<f64>, _> = text.split(',').map(super::helpers::number).collect();
            match numbers {
                Ok(v) if v.len() == 3 => Some([v[0], v[1], v[2]]),
                _ => return Err("level start needs x,y,z".into()),
            }
        }
    };
    def.levels.push(LevelDef { name: name.to_owned(), start });
    Ok(())
}

impl StateDef {
    pub(super) fn to_text(&self) -> String {
        if self.frozen { format!("state {} frozen=1", self.name) } else { format!("state {}", self.name) }
    }
}

impl LevelDef {
    pub(super) fn to_text(&self) -> String {
        match self.start {
            Some([x, y, z]) => format!("level {} start={x},{y},{z}", self.name),
            None => format!("level {}", self.name),
        }
    }
}

/// Every state or level a rule, action or spawn names must be declared.
pub(super) fn check(def: &GameDef) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let state_ok = |name: &str| def.states.iter().any(|state| state.name == name);
    let level_ok = |name: &str| def.levels.iter().any(|level| level.name == name);
    for rule in &def.rules {
        let mut complain = |message: String, hint: &str| out.push(Diagnostic::new(rule.line, 1, message, hint));
        if let Some(name) = &rule.state
            && !state_ok(name)
        {
            complain(format!("rule state '{name}' is not declared"), "declare it with: state NAME");
        }
        if let Some(name) = rule.event.strip_prefix("state:")
            && !state_ok(name)
        {
            complain(format!("event state:{name} names a state that is not declared"), "declare it with: state NAME");
        }
        if let Some(name) = rule.event.strip_prefix("level:")
            && !level_ok(name)
        {
            complain(format!("event level:{name} names a level that is not declared"), "declare it with: level NAME");
        }
        for action in &rule.actions {
            if let Some(name) = action.strip_prefix("go:").or_else(|| action.strip_prefix("restart:"))
                && !state_ok(name)
            {
                complain(format!("no state named '{name}' for {action}"), "declare it with: state NAME");
            }
            if let Some(name) = action.strip_prefix("level:")
                && !level_ok(name)
            {
                complain(format!("no level named '{name}' for {action}"), "declare it with: level NAME");
            }
        }
    }
    for spawn in &def.spawns {
        if let Some(name) = &spawn.level
            && !level_ok(name)
        {
            out.push(Diagnostic::new(1, 1, format!("spawn level '{name}' is not declared"), "declare it with: level NAME"));
        }
    }
    out
}
