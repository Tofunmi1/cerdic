//! The game methods of the runtime world API: what a script uses to play the pieces of a game it runs itself (a turn-based game, a
//! board, a puppet show). `game.entities` lists the entities of the loaded game, `game.spawn` puts one of a declared kind into the world,
//! `game.place` moves it (and turns it), `game.set_prop` changes one of its numbers, `game.remove` takes it away. An entity of a model kind
//! is drawn as its own copy of the animated model: moving it a little each frame makes the model walk, because its state machine is fed the
//! speed the entity really moves at. Headings are degrees about +Y, 0 facing +Z, as in `spawn ... heading=DEGREES`.
use bevy_ecs::world::World;
use serde_json::{Value, json};

use crate::game::GameRuntime;

/// Runs one `game.*` method.
pub(crate) fn game_method(
    world: &mut World,
    method: &str,
    params: &Value,
) -> Result<Value, String> {
    let mut runtime = world
        .get_resource_mut::<GameRuntime>()
        .ok_or("this scene has no game: declare one with game.kind and game.spawn nodes")?;
    let game = runtime.world.as_mut().ok_or(
        "no game is loaded yet: declare kinds with game.kind nodes and add a game.collect node",
    )?;
    match method {
        "game.entities" => {
            let kind = params.get("kind").and_then(Value::as_str);
            let entities: Vec<Value> = game
                .entities()
                .iter()
                .filter(|entity| kind.is_none_or(|kind| entity.kind == kind))
                .map(|entity| {
                    json!({
                        "id": entity.id,
                        "kind": entity.kind,
                        "position": entity.position,
                        "heading": entity.heading.to_degrees(),
                        "props": entity.props,
                    })
                })
                .collect();
            Ok(json!({ "entities": entities }))
        }
        "game.spawn" => {
            let kind = text(params, "kind", "game.spawn")?;
            let at = position(params, "game.spawn")?;
            let id = game
                .command_spawn(kind, at, degrees(params).unwrap_or(0.0))
                .map_err(|e| e.to_string())?;
            Ok(json!({ "id": id }))
        }
        "game.place" => {
            let id = id(params, "game.place")?;
            let at = position(params, "game.place")?;
            game.command_place(id, at, degrees(params))
                .map_err(|e| e.to_string())?;
            Ok(json!({ "id": id }))
        }
        "game.set_prop" => {
            let id = id(params, "game.set_prop")?;
            let name = text(params, "name", "game.set_prop")?;
            let value = params
                .get("value")
                .and_then(Value::as_f64)
                .ok_or("game.set_prop needs a numeric value")?;
            game.command_set_prop(id, name, value)
                .map_err(|e| e.to_string())?;
            Ok(json!({ "id": id }))
        }
        "game.remove" => {
            let id = id(params, "game.remove")?;
            game.command_remove(id).map_err(|e| e.to_string())?;
            Ok(json!({ "removed": id }))
        }
        other => Err(format!("unknown game method {other:?}")),
    }
}

fn text<'a>(params: &'a Value, key: &str, method: &str) -> Result<&'a str, String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{method} needs a {key} (a string)"))
}

fn id(params: &Value, method: &str) -> Result<u32, String> {
    params
        .get("id")
        .and_then(Value::as_u64)
        .and_then(|id| u32::try_from(id).ok())
        .ok_or_else(|| {
            format!("{method} needs an id (the number game.entities or game.spawn gave)")
        })
}

fn position(params: &Value, method: &str) -> Result<[f64; 3], String> {
    let values: Vec<f64> = params
        .get("position")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default();
    match values[..] {
        [x, y, z] => Ok([x, y, z]),
        _ => Err(format!("{method} needs a position [x, y, z]")),
    }
}

fn degrees(params: &Value) -> Option<f64> {
    params
        .get("heading")
        .and_then(Value::as_f64)
        .map(f64::to_radians)
}
