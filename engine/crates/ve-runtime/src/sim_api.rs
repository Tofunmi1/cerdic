//! The `sim.*` methods of the world API: what a model or a script can ask of the running simulation programs.
//!
//! - `sim.status {}` lists every instance: `{sims: [{part, mode, rate, elements, steps, params, states, bones, min, max, fingerprint, error}]}`.
//! - `sim.step {part?, steps}` advances the instances of the part (or all) by `steps` fixed steps now and answers as `sim.status` does.
//! - `sim.set_param {part?, name, values}` sets a `param` of a running program (`values` is one number or an array).
//! - `sim.reset {part?}` starts the instances again from their initial state.
use std::collections::HashSet;
use std::sync::Arc;

use bevy_ecs::world::World;
use serde_json::{Value, json};

use crate::animation::PartPoses;
use crate::projection::Authored;
use crate::sim_live::{self, Shared, SimLive};

fn instances(world: &mut World, part: Option<&str>) -> Vec<(String, Arc<Shared>)> {
    let mut query = world.query::<(&Authored, &SimLive)>();
    let mut seen: HashSet<*const Shared> = HashSet::new();
    let mut rows: Vec<(String, Arc<Shared>)> = query
        .iter(world)
        .filter(|(authored, _)| part.is_none_or(|p| authored.name() == p))
        .filter(|(_, live)| seen.insert(Arc::as_ptr(&live.shared)))
        .map(|(authored, live)| (authored.name().to_owned(), Arc::clone(&live.shared)))
        .collect();
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    rows
}

fn report(rows: &[(String, Arc<Shared>)]) -> Value {
    let list: Vec<Value> = rows
        .iter()
        .map(|(name, shared)| {
            let mut one = sim_live::describe(shared);
            one["part"] = json!(name);
            one
        })
        .collect();
    json!({ "sims": list })
}

pub(crate) fn method(world: &mut World, method: &str, params: &Value) -> Result<Value, String> {
    let part = params.get("part").and_then(Value::as_str);
    let rows = instances(world, part);
    if rows.is_empty() && method != "sim.status" {
        return Err(match part {
            Some(p) => format!("no running simulation on the part {p:?}; sim.status lists them"),
            None => "no simulation programs are running".to_owned(),
        });
    }
    match method {
        "sim.status" => Ok(report(&rows)),
        "sim.step" => {
            let steps = params.get("steps").and_then(Value::as_u64).ok_or("sim.step needs steps: a whole number")?;
            let steps = u32::try_from(steps).map_err(|_| "steps is too large")?;
            let poses = world.get_resource::<PartPoses>().ok_or("the animation poses are not ready")?;
            for (name, shared) in &rows {
                sim_live::step_now(shared, poses, steps).map_err(|e| format!("{name}: {e}"))?;
            }
            Ok(report(&rows))
        }
        "sim.set_param" => {
            let name = params.get("name").and_then(Value::as_str).ok_or("sim.set_param needs a name")?;
            let values: Vec<f32> = match params.get("values") {
                Some(Value::Array(items)) => items.iter().filter_map(Value::as_f64).map(|v| v as f32).collect(),
                Some(v) => v.as_f64().map(|n| vec![n as f32]).unwrap_or_default(),
                None => Vec::new(),
            };
            for (part_name, shared) in &rows {
                sim_live::set_param(shared, name, &values).map_err(|e| format!("{part_name}: {e}"))?;
            }
            Ok(report(&rows))
        }
        "sim.reset" => {
            for (_, shared) in &rows {
                sim_live::reset(shared);
            }
            Ok(report(&rows))
        }
        other => Err(format!("unknown method {other:?}; use sim.status, sim.step, sim.set_param or sim.reset")),
    }
}
