use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value as Json};

use super::{BrainError, Kind, MAX_DEPTH, Node, Scalar, State};

pub(super) fn build_root(
    source: &Json,
    library: &BTreeMap<String, String>,
) -> Result<Node, BrainError> {
    build(source, library, &mut BTreeSet::new(), 0)
}

fn field<'a>(map: &'a Map<String, Json>, key: &str) -> Result<&'a Json, BrainError> {
    map.get(key)
        .ok_or_else(|| BrainError::BadSpec(format!("missing field {key}")))
}
fn string(map: &Map<String, Json>, key: &str) -> Result<String, BrainError> {
    field(map, key)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| BrainError::BadSpec(format!("{key} must be text")))
}
fn number(map: &Map<String, Json>, key: &str) -> Result<f64, BrainError> {
    field(map, key)?
        .as_f64()
        .ok_or_else(|| BrainError::BadSpec(format!("{key} must be a number")))
}
fn scalar(value: &Json) -> Result<Scalar, BrainError> {
    match value {
        Json::Bool(v) => Ok(Scalar::Bool(*v)),
        Json::Number(v) => v
            .as_f64()
            .map(Scalar::Number)
            .ok_or_else(|| BrainError::BadSpec("invalid number".into())),
        Json::String(v) => Ok(Scalar::Text(v.clone())),
        _ => Err(BrainError::BadSpec(
            "value must be a number, bool or string".into(),
        )),
    }
}
fn child(
    map: &Map<String, Json>,
    library: &BTreeMap<String, String>,
    stack: &mut BTreeSet<String>,
    depth: usize,
) -> Result<Node, BrainError> {
    build(field(map, "child")?, library, stack, depth + 1)
}
fn children(
    map: &Map<String, Json>,
    library: &BTreeMap<String, String>,
    stack: &mut BTreeSet<String>,
    depth: usize,
) -> Result<Vec<Node>, BrainError> {
    let list = field(map, "children")?
        .as_array()
        .ok_or_else(|| BrainError::BadSpec("children must be an array".into()))?;
    if list.is_empty() {
        return Err(BrainError::BadSpec("children cannot be empty".into()));
    }
    list.iter()
        .map(|v| build(v, library, stack, depth + 1))
        .collect()
}
fn build(
    json: &Json,
    library: &BTreeMap<String, String>,
    stack: &mut BTreeSet<String>,
    depth: usize,
) -> Result<Node, BrainError> {
    if depth > MAX_DEPTH {
        return Err(BrainError::BadSpec(format!(
            "tree depth exceeds documented limit {MAX_DEPTH}"
        )));
    }
    let map = json
        .as_object()
        .ok_or_else(|| BrainError::BadSpec("node must be an object".into()))?;
    let typ = string(map, "type")?;
    let kind = match typ.as_str() {
        "sequence" => Kind::Sequence(children(map, library, stack, depth)?),
        "selector" => Kind::Selector(children(map, library, stack, depth)?),
        "parallel" => {
            let policy = string(map, "policy")?;
            let all = match policy.as_str() {
                "all" => true,
                "one" => false,
                _ => {
                    return Err(BrainError::BadSpec(
                        "parallel policy must be all or one".into(),
                    ));
                }
            };
            Kind::Parallel(all, children(map, library, stack, depth)?)
        }
        "inverter" => Kind::Inverter(Box::new(child(map, library, stack, depth)?)),
        "succeeder" => Kind::Succeeder(Box::new(child(map, library, stack, depth)?)),
        "repeat" => {
            let n = field(map, "times")?.as_u64().ok_or_else(|| {
                BrainError::BadSpec("times must be a non-negative integer".into())
            })?;
            let times =
                usize::try_from(n).map_err(|_| BrainError::BadSpec("times too large".into()))?;
            Kind::Repeat(times, Box::new(child(map, library, stack, depth)?))
        }
        "wait" => Kind::Wait(number(map, "secs")?),
        "cooldown" => Kind::Cooldown(
            number(map, "secs")?,
            Box::new(child(map, library, stack, depth)?),
        ),
        "condition" => Kind::Condition(string(map, "name")?),
        "action" => Kind::Action(string(map, "name")?),
        "check" => {
            let key = string(map, "key")?;
            let op = string(map, "op")?;
            if !["<", "<=", "==", "!=", ">=", ">"].contains(&op.as_str()) {
                return Err(BrainError::BadSpec(format!("invalid check operator {op}")));
            }
            Kind::Check(key, op, scalar(field(map, "value")?)?)
        }
        "set" => Kind::Set(string(map, "key")?, scalar(field(map, "value")?)?),
        "subtree" => return build_subtree(map, library, stack, depth),
        _ => return Err(BrainError::BadSpec(format!("unknown node type {typ}"))),
    };
    Ok(Node {
        kind,
        state: State::default(),
    })
}
fn build_subtree(
    map: &Map<String, Json>,
    library: &BTreeMap<String, String>,
    stack: &mut BTreeSet<String>,
    depth: usize,
) -> Result<Node, BrainError> {
    let name = string(map, "name")?;
    if !stack.insert(name.clone()) {
        return Err(BrainError::Cycle(name));
    }
    let result = match library.get(&name) {
        Some(text) => {
            let parsed = serde_json::from_str(text)
                .map_err(|e| BrainError::BadSpec(format!("subtree {name}: {e}")))?;
            build(&parsed, library, stack, depth + 1)
        }
        None => Err(BrainError::BadSpec(format!("missing subtree {name}"))),
    };
    stack.remove(&name);
    result
}
