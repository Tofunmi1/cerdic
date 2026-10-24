//! JSON command decoding and response encoding.
use super::{HostError, NpcHost, NpcSpec};
use crate::{RequestId, Response};
use serde_json::{Value, json};

/// Executes the host's JSON command surface without panicking on malformed input.
pub(super) fn handle(host: &mut NpcHost, input: &str) -> String {
    let result = serde_json::from_str::<Value>(input)
        .map_err(|e| HostError::BadCommand(e.to_string()))
        .and_then(|v| execute(host, v));
    let output = result.unwrap_or_else(|e| json!({"ok": false, "error": e.to_string()}));
    serde_json::to_string(&output)
        .unwrap_or_else(|_| "{\"ok\":false,\"error\":\"serialization failed\"}".into())
}

fn execute(host: &mut NpcHost, v: Value) -> Result<Value, HostError> {
    let obj = v
        .as_object()
        .ok_or_else(|| HostError::BadCommand("command must be an object".into()))?;
    let cmd = field(obj, "cmd")?
        .as_str()
        .ok_or_else(|| HostError::BadCommand("cmd must be text".into()))?;
    match cmd {
        "register" => register(host, obj),
        "add_secret" => add_secret(host, obj),
        "set_examples" => set_examples(host, obj),
        "set_deflections" => set_deflections(host, obj),
        "set_filler" => set_filler(host, obj),
        "maybe_reflect" => maybe_reflect(host, obj),
        "set_reflection_threshold" => {
            host.set_reflection_threshold(number(obj, "value")?.to_string().parse().map_err(bad)?);
            Ok(json!({"ok":true}))
        }
        "observe" => observe(host, obj),
        "player_says" => player_says(host, obj),
        "take_outbox" => Ok(outbox(host)),
        "deliver" => deliver(host, obj),
        "tick" => tick(host, obj),
        "memory" => memory(host, obj),
        "transcript" => transcript(host, obj),
        "events" => events(host, obj),
        "save" => Ok(json!({"ok":true,"save":host.to_json()})),
        _ => Err(HostError::BadCommand(format!("unknown command {cmd}"))),
    }
}
fn set_filler(
    host: &mut NpcHost,
    obj: &serde_json::Map<String, Value>,
) -> Result<Value, HostError> {
    let spec = super::FillerSpec {
        after_secs: number(obj, "after_secs")?,
        lines: serde_json::from_value(field(obj, "lines")?.clone()).map_err(bad)?,
    };
    host.set_filler(string(obj, "npc")?, spec);
    Ok(json!({"ok":true}))
}
fn maybe_reflect(
    host: &mut NpcHost,
    obj: &serde_json::Map<String, Value>,
) -> Result<Value, HostError> {
    let asked = host.maybe_reflect(string(obj, "npc")?, number(obj, "now")?)?;
    Ok(json!({"ok":true,"asked":asked}))
}
fn add_secret(
    host: &mut NpcHost,
    obj: &serde_json::Map<String, Value>,
) -> Result<Value, HostError> {
    let secret = serde_json::from_value(field(obj, "secret")?.clone()).map_err(bad)?;
    host.add_secret(string(obj, "npc")?, secret)?;
    Ok(json!({"ok":true}))
}
fn set_examples(
    host: &mut NpcHost,
    obj: &serde_json::Map<String, Value>,
) -> Result<Value, HostError> {
    let raw = field(obj, "examples")?
        .as_array()
        .ok_or_else(|| HostError::BadCommand("examples must be an array".into()))?;
    let mut examples = Vec::new();
    for row in raw.iter().take(8) {
        let pair = row
            .as_array()
            .filter(|a| a.len() == 2)
            .ok_or_else(|| HostError::BadCommand("each example must be a pair".into()))?;
        examples.push((
            pair[0]
                .as_str()
                .ok_or_else(|| HostError::BadCommand("player example must be text".into()))?
                .into(),
            pair[1]
                .as_str()
                .ok_or_else(|| HostError::BadCommand("NPC example must be text".into()))?
                .into(),
        ));
    }
    host.set_examples(string(obj, "npc")?, examples)?;
    Ok(json!({"ok":true}))
}
fn set_deflections(
    host: &mut NpcHost,
    obj: &serde_json::Map<String, Value>,
) -> Result<Value, HostError> {
    let lines = serde_json::from_value(field(obj, "lines")?.clone()).map_err(bad)?;
    host.set_deflections(string(obj, "npc")?, lines);
    Ok(json!({"ok":true}))
}

fn register(host: &mut NpcHost, obj: &serde_json::Map<String, Value>) -> Result<Value, HostError> {
    let spec: NpcSpec = serde_json::from_value(field(obj, "npc")?.clone()).map_err(bad)?;
    host.register(spec)?;
    Ok(json!({"ok":true}))
}
fn observe(host: &mut NpcHost, obj: &serde_json::Map<String, Value>) -> Result<Value, HostError> {
    let importance = number(obj, "importance")?
        .clamp(0.0, 1.0)
        .to_string()
        .parse::<f32>()
        .unwrap_or(0.0);
    let id = host.observe(
        string(obj, "npc")?,
        string(obj, "text")?,
        importance,
        number(obj, "now")?,
    )?;
    Ok(json!({"ok":true,"id":id.0}))
}
fn player_says(
    host: &mut NpcHost,
    obj: &serde_json::Map<String, Value>,
) -> Result<Value, HostError> {
    let refused = host.player_says(
        string(obj, "npc")?,
        string(obj, "text")?,
        number(obj, "now")?,
    )?;
    let name = match refused {
        None => Value::Null,
        Some(crate::Refusal::RateLimited) => json!("rate_limited"),
        Some(crate::Refusal::OverTokenBudget) => json!("over_token_budget"),
    };
    Ok(json!({"ok":true,"refused":name}))
}
fn outbox(host: &mut NpcHost) -> Value {
    let requests = host.take_outbox().into_iter().map(|r| json!({"id":r.id.0,"npc":r.npc,"prompt":r.prompt,"priority":r.priority,"reason":format!("{:?}",r.reason)})).collect::<Vec<_>>();
    json!({"ok":true,"requests":requests})
}
fn deliver(host: &mut NpcHost, obj: &serde_json::Map<String, Value>) -> Result<Value, HostError> {
    let id = RequestId(integer(obj, "id")?);
    let text = string(obj, "text")?.to_owned();
    let response = Response {
        id,
        outcome: if boolean(obj, "ok")? {
            Ok(text)
        } else {
            Err(text)
        },
    };
    let delivery = host
        .deliver(response, number(obj, "now")?)
        .ok_or_else(|| HostError::BadCommand("unknown request id".into()))?;
    let events = host.drain_events(&delivery.npc)?;
    Ok(json!({"ok":true,"fallback":delivery.fallback,"events":events}))
}
fn tick(host: &mut NpcHost, obj: &serde_json::Map<String, Value>) -> Result<Value, HostError> {
    Ok(json!({"ok":true,"deliveries":host.tick(number(obj,"now")?).len()}))
}
fn memory(host: &NpcHost, obj: &serde_json::Map<String, Value>) -> Result<Value, HostError> {
    let npc = string(obj, "npc")?;
    let store = host
        .npcs
        .get(npc)
        .ok_or_else(|| HostError::UnknownNpc(npc.into()))?;
    let k = usize::try_from(integer(obj, "k")?).map_err(bad)?;
    let rows = store.memory.rank(string(obj,"query")?,number(obj,"now")?,k,&host.weights,&host.embedder).into_iter().filter_map(|r| store.memory.get(r.id).map(|m| json!({"id":m.id.0,"text":m.text,"kind":format!("{:?}",m.kind),"importance":m.importance,"score":r.score}))).collect::<Vec<_>>();
    Ok(json!({"ok":true,"memories":rows}))
}
fn transcript(host: &NpcHost, obj: &serde_json::Map<String, Value>) -> Result<Value, HostError> {
    let npc = string(obj, "npc")?;
    let session = host
        .session(npc)
        .ok_or_else(|| HostError::UnknownNpc(npc.into()))?;
    Ok(json!({"ok":true,"turns":session.transcript()}))
}
fn events(host: &mut NpcHost, obj: &serde_json::Map<String, Value>) -> Result<Value, HostError> {
    Ok(json!({"ok":true,"events":host.drain_events(string(obj,"npc")?)?}))
}
fn field<'a>(o: &'a serde_json::Map<String, Value>, k: &str) -> Result<&'a Value, HostError> {
    o.get(k)
        .ok_or_else(|| HostError::BadCommand(format!("missing field {k}")))
}
fn string<'a>(o: &'a serde_json::Map<String, Value>, k: &str) -> Result<&'a str, HostError> {
    field(o, k)?
        .as_str()
        .ok_or_else(|| HostError::BadCommand(format!("{k} must be text")))
}
fn number(o: &serde_json::Map<String, Value>, k: &str) -> Result<f64, HostError> {
    field(o, k)?
        .as_f64()
        .filter(|v| v.is_finite())
        .ok_or_else(|| HostError::BadCommand(format!("{k} must be a finite number")))
}
fn integer(o: &serde_json::Map<String, Value>, k: &str) -> Result<u64, HostError> {
    field(o, k)?
        .as_u64()
        .ok_or_else(|| HostError::BadCommand(format!("{k} must be a nonnegative integer")))
}
fn boolean(o: &serde_json::Map<String, Value>, k: &str) -> Result<bool, HostError> {
    field(o, k)?
        .as_bool()
        .ok_or_else(|| HostError::BadCommand(format!("{k} must be boolean")))
}
fn bad(e: impl std::fmt::Display) -> HostError {
    HostError::BadCommand(e.to_string())
}
