//! FSM JSON parsing, linking, and serialization.
use super::{Machine, State, Transition, When};
use crate::brain::tree::Scalar;
use crate::{BrainError, Registry};
use serde_json::{Map, Value as Json};
use std::collections::{BTreeMap, BTreeSet};

type Result<T> = std::result::Result<T, BrainError>;
fn bad(message: impl Into<String>) -> BrainError {
    BrainError::BadSpec(message.into())
}
fn text<'a>(obj: &'a Map<String, Json>, key: &str) -> Result<&'a str> {
    obj.get(key)
        .and_then(Json::as_str)
        .ok_or_else(|| bad(format!("{key} must be text")))
}
fn object<'a>(value: &'a Json, label: &str) -> Result<&'a Map<String, Json>> {
    value
        .as_object()
        .ok_or_else(|| bad(format!("{label} must be an object")))
}

pub(super) fn parse(source: &str, library: &BTreeMap<String, String>) -> Result<Machine> {
    let json: Json = serde_json::from_str(source).map_err(|e| bad(e.to_string()))?;
    build(&json, library, &mut BTreeSet::new(), 0)
}
fn build(
    json: &Json,
    library: &BTreeMap<String, String>,
    stack: &mut BTreeSet<String>,
    depth: usize,
) -> Result<Machine> {
    if depth > 64 {
        return Err(bad("machine nesting exceeds 64"));
    }
    let obj = object(json, "machine")?;
    let initial = text(obj, "initial")?.to_owned();
    let raw_states = obj
        .get("states")
        .and_then(Json::as_object)
        .ok_or_else(|| bad("states must be a non-empty object"))?;
    if raw_states.is_empty() || !raw_states.contains_key(&initial) {
        return Err(bad("initial must name a state and states cannot be empty"));
    }
    let mut states = BTreeMap::new();
    for (name, raw) in raw_states {
        let item = object(raw, "state")?;
        let link = item.get("link");
        let nested = item.get("states");
        if link.is_some() && nested.is_some() {
            return Err(bad(format!("state {name} has both link and states")));
        }
        let child = if let Some(link) = link {
            let linked = link.as_str().ok_or_else(|| bad("link must be text"))?;
            if !stack.insert(linked.to_owned()) {
                return Err(bad(format!("link cycle involving {linked}")));
            }
            let resolved = library
                .get(linked)
                .ok_or_else(|| bad(format!("missing linked machine {linked}")))
                .and_then(|s| parse_link(s, library, stack, depth + 1));
            stack.remove(linked);
            Some(Box::new(resolved?))
        } else if nested.is_some() {
            Some(Box::new(build(raw, library, stack, depth + 1)?))
        } else {
            None
        };
        let transitions = item
            .get("transitions")
            .map(|v| {
                v.as_array()
                    .ok_or_else(|| bad("transitions must be an array"))
            })
            .transpose()?
            .map_or(&[][..], |v| v.as_slice())
            .iter()
            .map(|v| {
                let t = object(v, "transition")?;
                let to = text(t, "to")?.to_owned();
                if !raw_states.contains_key(&to) {
                    return Err(bad(format!(
                        "transition target {to} is not a sibling state"
                    )));
                }
                let when = t
                    .get("when")
                    .ok_or_else(|| bad("transition missing when"))?;
                Ok(Transition {
                    to,
                    when: parse_when(when, depth + 1)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        states.insert(
            name.clone(),
            State {
                enter: optional_text(item, "on_enter")?,
                update: optional_text(item, "on_update")?,
                exit: optional_text(item, "on_exit")?,
                transitions,
                child,
            },
        );
    }
    Ok(Machine { initial, states })
}
fn parse_link(
    source: &str,
    library: &BTreeMap<String, String>,
    stack: &mut BTreeSet<String>,
    depth: usize,
) -> Result<Machine> {
    let json: Json = serde_json::from_str(source).map_err(|e| bad(e.to_string()))?;
    build(&json, library, stack, depth)
}
fn optional_text(item: &Map<String, Json>, key: &str) -> Result<Option<String>> {
    item.get(key)
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or_else(|| bad(format!("{key} must be text")))
        })
        .transpose()
}
fn parse_when(json: &Json, depth: usize) -> Result<When> {
    if depth > 64 {
        return Err(bad("condition nesting exceeds 64"));
    }
    let obj = object(json, "when")?;
    if obj.len() != 1 {
        return Err(bad("when must have exactly one type"));
    }
    let (kind, value) = obj.iter().next().ok_or_else(|| bad("empty when"))?;
    Ok(match kind.as_str() {
        "condition" => When::Condition(
            value
                .as_str()
                .ok_or_else(|| bad("condition must be text"))?
                .to_owned(),
        ),
        "after" => When::After(
            value
                .as_f64()
                .filter(|n| n.is_finite() && *n >= 0.0)
                .ok_or_else(|| bad("after must be non-negative seconds"))?,
        ),
        "event" => When::Event(
            value
                .as_str()
                .ok_or_else(|| bad("event must be text"))?
                .to_owned(),
        ),
        "not" => When::Not(Box::new(parse_when(value, depth + 1)?)),
        "all" | "any" => {
            let values = value
                .as_array()
                .ok_or_else(|| bad(format!("{kind} must be an array")))?;
            let items = values
                .iter()
                .map(|v| parse_when(v, depth + 1))
                .collect::<Result<Vec<_>>>()?;
            if kind == "all" {
                When::All(items)
            } else {
                When::Any(items)
            }
        }
        "check" => {
            let c = object(value, "check")?;
            let key = text(c, "key")?.to_owned();
            let op = text(c, "op")?.to_owned();
            if !["<", "<=", "==", "!=", ">=", ">"].contains(&op.as_str()) {
                return Err(bad(format!("invalid check operator {op}")));
            }
            let expected = match c.get("value") {
                Some(Json::Bool(v)) => Scalar::Bool(*v),
                Some(Json::Number(v)) => {
                    Scalar::Number(v.as_f64().ok_or_else(|| bad("invalid check value"))?)
                }
                Some(Json::String(v)) => Scalar::Text(v.clone()),
                _ => return Err(bad("check value must be number, bool or text")),
            };
            When::Check(key, op, expected)
        }
        _ => return Err(bad(format!("unknown when type {kind}"))),
    })
}

pub(super) fn collect(machine: &Machine, registry: &Registry, names: &mut BTreeSet<String>) {
    for state in machine.states.values() {
        for action in [&state.enter, &state.update, &state.exit]
            .into_iter()
            .flatten()
        {
            if !registry.has_action(action) {
                names.insert(action.clone());
            }
        }
        for transition in &state.transitions {
            collect_when(&transition.when, registry, names);
        }
        if let Some(child) = &state.child {
            collect(child, registry, names);
        }
    }
}
fn collect_when(when: &When, registry: &Registry, names: &mut BTreeSet<String>) {
    match when {
        When::Condition(n) if !registry.has_condition(n) => {
            names.insert(n.clone());
        }
        When::Not(v) => collect_when(v, registry, names),
        When::All(v) | When::Any(v) => {
            for item in v {
                collect_when(item, registry, names);
            }
        }
        _ => {}
    }
}

pub(super) fn serialize(machine: &Machine) -> String {
    fn machine_json(machine: &Machine) -> Json {
        let mut states = serde_json::Map::new();
        for (name, state) in &machine.states {
            let mut obj = serde_json::Map::new();
            for (key, value) in [
                ("on_enter", &state.enter),
                ("on_update", &state.update),
                ("on_exit", &state.exit),
            ] {
                if let Some(value) = value {
                    obj.insert(key.into(), Json::String(value.clone()));
                }
            }
            if !state.transitions.is_empty() {
                obj.insert(
                    "transitions".into(),
                    Json::Array(
                        state
                            .transitions
                            .iter()
                            .map(|t| serde_json::json!({"to":t.to,"when":when_json(&t.when)}))
                            .collect(),
                    ),
                );
            }
            if let Some(child) = &state.child {
                obj.insert("initial".into(), Json::String(child.initial.clone()));
                obj.insert("states".into(), machine_json(child)["states"].clone());
            }
            states.insert(name.clone(), Json::Object(obj));
        }
        serde_json::json!({"initial":machine.initial,"states":states})
    }
    fn when_json(when: &When) -> Json {
        match when {
            When::Condition(v) => serde_json::json!({"condition":v}),
            When::After(v) => serde_json::json!({"after":v}),
            When::Event(v) => serde_json::json!({"event":v}),
            When::Not(v) => serde_json::json!({"not":when_json(v)}),
            When::All(v) => serde_json::json!({"all":v.iter().map(when_json).collect::<Vec<_>>()}),
            When::Any(v) => serde_json::json!({"any":v.iter().map(when_json).collect::<Vec<_>>()}),
            When::Check(k, o, v) => {
                let value = match v {
                    Scalar::Bool(b) => Json::Bool(*b),
                    Scalar::Number(n) => serde_json::json!(n),
                    Scalar::Text(s) => Json::String(s.clone()),
                };
                serde_json::json!({"check":{"key":k,"op":o,"value":value}})
            }
        }
    }
    machine_json(machine).to_string()
}
