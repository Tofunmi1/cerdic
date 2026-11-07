//! The runtime world API (ADR 0065): the running world, reachable by component name, for models, tools and scripts.
//!
//! The page calls `world_call(method, params_json)` and gets a request id; at the start of the next frame the runtime answers every
//! queued request (at most `MAX_REQUESTS_PER_FRAME`) and keeps the answers for `world_take(id)`. Method names and JSON shapes follow
//! `bevy_remote` where it has the method. Only components that are registered for reflection can be read; only those on `WRITABLE` can
//! be changed (a reflected write to an immutable component panics, and a panic ends the app, so writes are opt-in per component).
use std::any::TypeId;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};

use bevy_ecs::{
    entity::Entity,
    reflect::{AppTypeRegistry, ReflectComponent},
    world::World,
};
use bevy_reflect::{
    PartialReflect, TypeRegistration, TypeRegistry,
    serde::{TypedReflectDeserializer, TypedReflectSerializer},
};
use serde::de::DeserializeSeed;
use serde_json::{Map, Value, json};
use wasm_bindgen::prelude::wasm_bindgen;

use crate::projection::Authored;

/// Requests answered per frame; the rest wait for the next one.
const MAX_REQUESTS_PER_FRAME: usize = 64;
/// Rows one `world.query` may return.
const MAX_QUERY_ROWS: usize = 1_000;
/// Answers kept for `world_take`; the oldest are dropped past this.
const MAX_KEPT_ANSWERS: usize = 256;
/// Bytes one request's parameters may take.
const MAX_PARAMS_BYTES: usize = 1 << 20;
/// Components a request may change (short type paths). Everything else is read only until it has been checked to be safe to write.
/// (`Name` is readable only: its hashed string type has no reflected deserializer.)
const WRITABLE: &[&str] = &[
    "Transform",
    "Visibility",
    "PointLight",
    "SpotLight",
    "DirectionalLight",
    "VeMaterial",
];

thread_local! {
    static REQUESTS: RefCell<VecDeque<(u32, String, String)>> = const { RefCell::new(VecDeque::new()) };
    static ANSWERS: RefCell<HashMap<u32, String>> = RefCell::new(HashMap::new());
    static NEXT_ID: Cell<u32> = const { Cell::new(1) };
}

/// Ragdolls that behaviours (`behavior.on action="ragdoll"`) asked to start, by `phys.ragdoll` name; started at the next frame.
const MAX_RAGDOLL_REQUESTS: usize = 64;

thread_local! {
    static RAGDOLL_REQUESTS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Asks for the character whose `phys.ragdoll` is called `name` to go limp (a fired `behavior.on action="ragdoll"`).
pub(crate) fn request_ragdoll(name: &str) {
    RAGDOLL_REQUESTS.with(|queue| {
        let mut queue = queue.borrow_mut();
        if queue.len() < MAX_RAGDOLL_REQUESTS {
            queue.push(name.to_owned());
        }
    });
}

/// Events kept for scripts and tools (the oldest are dropped past this).
const MAX_EVENTS: usize = 256;

thread_local! {
    static EVENTS: RefCell<VecDeque<(u64, Value)>> = const { RefCell::new(VecDeque::new()) };
    static EVENT_SEQ: Cell<u64> = const { Cell::new(0) };
}

/// Records something that happened in the world, for scripts: `name` is what happened (`click`, `hover`, `unhover`, `press`, `release`,
/// `grab`, `drop`, or `game.<effect kind>`), `subject` the part or effect name it concerns, `extra` any more detail. A script listens with
/// `on("click", ...)` or, for one part, `on("click:crate_left", ...)`.
thread_local! {
    /// Animation events (name, game entity) waiting for the game's next step, which raises each as `host:anim.<name>`.
    static HOST_EVENTS: std::cell::RefCell<Vec<(String, Option<u32>)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Queues an animation event for the game. `part` is the machine's part name: a game unit's copy is named `<name>#<id>`, which says which entity it is.
pub(crate) fn queue_host_event(name: &str, part: &str) {
    let entity = part
        .rsplit_once('#')
        .and_then(|(_, id)| id.parse::<u32>().ok());
    HOST_EVENTS.with(|queue| {
        let mut queue = queue.borrow_mut();
        // A scene full of walkers must not grow this without bound while the game is not stepping.
        if queue.len() < 256 {
            queue.push((name.to_owned(), entity));
        }
    });
}

/// Takes the queued animation events.
pub(crate) fn take_host_events() -> Vec<(String, Option<u32>)> {
    HOST_EVENTS.with(|queue| std::mem::take(&mut *queue.borrow_mut()))
}

pub(crate) fn push_event(name: &str, subject: &str, extra: Value) {
    let seq = EVENT_SEQ.with(|seq| {
        seq.set(seq.get() + 1);
        seq.get()
    });
    let mut event = json!({ "seq": seq, "name": name, "part": subject });
    if let (Some(map), Some(more)) = (event.as_object_mut(), extra.as_object()) {
        for (key, value) in more {
            map.entry(key.clone()).or_insert_with(|| value.clone());
        }
    }
    EVENTS.with(|events| {
        let mut events = events.borrow_mut();
        events.push_back((seq, event));
        while events.len() > MAX_EVENTS {
            events.pop_front();
        }
    });
}

/// The events after sequence number `after` (0 for all kept), as `{"events": [...], "last": n}`.
#[wasm_bindgen]
#[must_use]
pub fn world_events_since(after: f64) -> String {
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a sequence number well inside f64's exact range"
    )]
    let after = after.max(0.0) as u64;
    EVENTS.with(|events| {
        let events = events.borrow();
        let fresh: Vec<&Value> = events
            .iter()
            .filter(|(seq, _)| *seq > after)
            .map(|(_, event)| event)
            .collect();
        let last = events.back().map_or(after, |(seq, _)| *seq.max(&after));
        json!({ "events": fresh, "last": last }).to_string()
    })
}

/// Queues a call; the answer is ready after the next frame. Returns the request id (0 when the queue is full).
#[wasm_bindgen]
pub fn world_call(method: &str, params: &str) -> u32 {
    REQUESTS.with(|queue| {
        let mut queue = queue.borrow_mut();
        if queue.len() >= 1024 {
            return 0;
        }
        let id = NEXT_ID.with(|next| {
            let id = next.get();
            next.set(id.wrapping_add(1).max(1));
            id
        });
        queue.push_back((id, method.to_owned(), params.to_owned()));
        id
    })
}

/// The answer to a request (`{"ok":true,"result":...}` or `{"ok":false,"error":"..."}`), once, or nothing while it is still waiting.
#[wasm_bindgen]
#[must_use]
pub fn world_take(id: u32) -> Option<String> {
    ANSWERS.with(|answers| answers.borrow_mut().remove(&id))
}

/// Answers the queued requests; an exclusive system, run once a frame.
pub(crate) fn run(world: &mut World) {
    let limp: Vec<String> = RAGDOLL_REQUESTS.with(|queue| std::mem::take(&mut *queue.borrow_mut()));
    for name in limp {
        // A character already limp, or physics not running, is not an error for the behaviour that asked.
        if physics_ragdoll(world, &json!({ "name": name })).is_ok() {
            push_event("ragdoll", &name, Value::Null);
        }
    }
    let batch: Vec<(u32, String, String)> = REQUESTS.with(|queue| {
        let mut queue = queue.borrow_mut();
        let count = queue.len().min(MAX_REQUESTS_PER_FRAME);
        queue.drain(..count).collect()
    });
    for (id, method, params) in batch {
        let answer = match handle(world, &method, &params) {
            Ok(result) => json!({ "ok": true, "result": result }),
            Err(error) => json!({ "ok": false, "error": error }),
        };
        ANSWERS.with(|answers| {
            let mut answers = answers.borrow_mut();
            answers.insert(id, answer.to_string());
            while answers.len() > MAX_KEPT_ANSWERS {
                let oldest = answers.keys().copied().min();
                match oldest {
                    Some(key) => answers.remove(&key),
                    None => break,
                };
            }
        });
    }
}

fn handle(world: &mut World, method: &str, params: &str) -> Result<Value, String> {
    if params.len() > MAX_PARAMS_BYTES {
        return Err(format!(
            "the parameters are {} bytes; the limit is {MAX_PARAMS_BYTES}",
            params.len()
        ));
    }
    let params: Value = if params.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(params)
            .map_err(|e| format!("the parameters are not valid JSON: {e}"))?
    };
    match method {
        "world.batch" => batch(world, &params),
        "world.bounds" => bounds(world, &params),
        "fx.burst" => fx_burst(&params),
        "fx.status" => Ok(crate::effects::status(world)),
        "physics.raycast" => physics_raycast(world, &params),
        "physics.overlap" => physics_overlap(world, &params),
        "physics.ragdoll" => physics_ragdoll(world, &params),
        "water.disturb" | "water.height" | "water.status" => water_method(world, method, &params),
        "sim.status" | "sim.step" | "sim.set_param" | "sim.reset" => crate::sim_api::method(world, method, &params),
        "ground.sample" | "ground.profile" => ground_query(world, method, &params),
        "animation.set_float" | "animation.set_bool" | "animation.trigger" => {
            animation_method(method, &params)
        }
        "game.entities" | "game.spawn" | "game.place" | "game.set_prop" | "game.remove" => {
            crate::world_api_game::game_method(world, method, &params)
        }
        "registry.list" => registry_list(world),
        "world.parts" => Ok(parts(world)),
        "world.find" => find(world, &params),
        "world.list_components" => list_components(world, &params),
        "world.get_components" => get_components(world, &params),
        "world.insert_components" | "world.mutate_components" => insert_components(world, &params),
        "world.query" => query(world, &params),
        "world.spawn_entity" => spawn_entity(world, &params),
        "world.despawn_entity" => despawn_entity(world, &params),
        _ => Err(format!(
            "unknown method {method:?}; use game.entities, game.spawn, game.place, game.set_prop, game.remove, registry.list, world.parts, world.find, world.list_components, world.get_components, world.insert_components (or world.mutate_components), world.query, world.spawn_entity, world.despawn_entity, world.bounds, world.batch, animation.set_float, animation.set_bool, animation.trigger, fx.burst, fx.status, physics.raycast, physics.overlap, physics.ragdoll, ground.sample or ground.profile"
        )),
    }
}

/// Operations one `world.batch` may carry.
const MAX_BATCH_OPS: usize = 512;

/// Several calls in one request, answered in order: `{ops: [{method, params}, ...]}` gives `{results: [{ok, result | error}, ...]}`. A script
/// that moves many parts every frame sends one batch, so the per-frame request budget is spent once. A batch cannot contain a batch.
fn batch(world: &mut World, params: &Value) -> Result<Value, String> {
    let ops = params
        .get("ops")
        .and_then(Value::as_array)
        .ok_or("world.batch needs an ops list of {method, params}")?;
    if ops.len() > MAX_BATCH_OPS {
        return Err(format!(
            "a batch carries at most {MAX_BATCH_OPS} operations, not {}",
            ops.len()
        ));
    }
    let results: Vec<Value> = ops
        .iter()
        .map(|op| {
            let method = op.get("method").and_then(Value::as_str).unwrap_or("");
            if method == "world.batch" {
                return json!({ "ok": false, "error": "a batch cannot contain a batch" });
            }
            let text = op.get("params").map(Value::to_string).unwrap_or_default();
            match handle(world, method, &text) {
                Ok(result) => json!({ "ok": true, "result": result }),
                Err(error) => json!({ "ok": false, "error": error }),
            }
        })
        .collect();
    Ok(json!({ "results": results }))
}

fn registry_arc(world: &World) -> Result<AppTypeRegistry, String> {
    world
        .get_resource::<AppTypeRegistry>()
        .cloned()
        .ok_or_else(|| "the world has no type registry".to_owned())
}

fn entity_text(entity: Entity) -> String {
    entity.to_bits().to_string()
}

/// An entity from its id string, or from an authored part name (the first part with that name).
fn entity_of(world: &mut World, value: &Value) -> Result<Entity, String> {
    if let Some(text) = value.as_str() {
        if let Ok(bits) = text.parse::<u64>() {
            let entity = Entity::try_from_bits(bits)
                .ok_or_else(|| format!("{text:?} is not an entity id"))?;
            return if world.get_entity(entity).is_ok() {
                Ok(entity)
            } else {
                Err(format!(
                    "no entity {text} exists (it may have been despawned)"
                ))
            };
        }
        let mut query = world.query::<(Entity, &Authored)>();
        let found: Vec<Entity> = query
            .iter(world)
            .filter(|(_, authored)| authored.name() == text)
            .map(|(entity, _)| entity)
            .collect();
        return match found.as_slice() {
            [one] => Ok(*one),
            [] => Err(format!(
                "no part is named {text:?}; call world.parts for the names"
            )),
            many => Err(format!(
                "{} entities are named {text:?} (a placed set draws one entity per copy); use an id from world.find",
                many.len()
            )),
        };
    }
    Err("an entity is given as the string of its id, or as a part name".to_owned())
}

fn registration<'a>(
    registry: &'a TypeRegistry,
    name: &str,
) -> Result<&'a TypeRegistration, String> {
    registry
        .get_with_type_path(name)
        .or_else(|| registry.get_with_short_type_path(name))
        .ok_or_else(|| format!("no component named {name:?} is registered for reflection; call registry.list for the names"))
}

fn short_name(registration: &TypeRegistration) -> &str {
    registration.type_info().type_path_table().short_path()
}

fn registry_list(world: &World) -> Result<Value, String> {
    let app = registry_arc(world)?;
    let registry = app.read();
    let mut rows: Vec<Value> = registry
        .iter()
        .filter(|r| r.data::<ReflectComponent>().is_some())
        .map(|r| json!({ "name": r.type_info().type_path(), "short": short_name(r), "writable": WRITABLE.contains(&short_name(r)) }))
        .collect();
    rows.sort_by_key(|row| row["short"].as_str().unwrap_or("").to_owned());
    rows.truncate(2_000);
    Ok(json!({ "components": rows }))
}

fn parts(world: &mut World) -> Value {
    let mut query = world.query::<(Entity, &Authored)>();
    let mut rows: Vec<Value> = query
        .iter(world)
        .map(|(entity, authored)| json!({ "entity": entity_text(entity), "name": authored.name(), "node": authored.node() }))
        .collect();
    rows.sort_by_key(|row| row["name"].as_str().unwrap_or("").to_owned());
    rows.truncate(MAX_QUERY_ROWS);
    json!({ "parts": rows })
}

fn find(world: &mut World, params: &Value) -> Result<Value, String> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or("world.find needs a name")?;
    let mut query = world.query::<(Entity, &Authored)>();
    let rows: Vec<Value> = query
        .iter(world)
        .filter(|(_, authored)| authored.name() == name)
        .take(MAX_QUERY_ROWS)
        .map(|(entity, authored)| json!({ "entity": entity_text(entity), "name": authored.name(), "node": authored.node() }))
        .collect();
    Ok(json!({ "entities": rows }))
}

fn list_components(world: &mut World, params: &Value) -> Result<Value, String> {
    let entity = entity_of(
        world,
        params
            .get("entity")
            .ok_or("world.list_components needs an entity")?,
    )?;
    let app = registry_arc(world)?;
    let registry = app.read();
    let entity_ref = world.entity(entity);
    let mut names: Vec<String> = registry
        .iter()
        .filter(|r| {
            r.data::<ReflectComponent>()
                .is_some_and(|c| c.contains(entity_ref))
        })
        .map(|r| r.type_info().type_path().to_owned())
        .collect();
    names.sort();
    Ok(json!({ "entity": entity_text(entity), "components": names }))
}

fn get_components(world: &mut World, params: &Value) -> Result<Value, String> {
    let entity = entity_of(
        world,
        params
            .get("entity")
            .ok_or("world.get_components needs an entity")?,
    )?;
    let names = string_list(params.get("components"), "components")?;
    if names.is_empty() {
        return Err("world.get_components needs a list of component names".to_owned());
    }
    let app = registry_arc(world)?;
    let registry = app.read();
    let mut out = Map::new();
    let mut errors = Map::new();
    for name in &names {
        let found = registration(&registry, name).and_then(|r| {
            let component = r
                .data::<ReflectComponent>()
                .ok_or_else(|| format!("{name:?} is not a component"))?;
            let value = component
                .reflect(world.entity(entity))
                .ok_or_else(|| format!("the entity has no {name}"))?;
            serde_json::to_value(TypedReflectSerializer::new(value, &registry))
                .map_err(|e| e.to_string())
        });
        match found {
            Ok(value) => out.insert(name.clone(), value),
            Err(error) => errors.insert(name.clone(), Value::String(error)),
        };
    }
    Ok(json!({ "entity": entity_text(entity), "components": out, "errors": errors }))
}

fn insert_components(world: &mut World, params: &Value) -> Result<Value, String> {
    let entity = entity_of(
        world,
        params
            .get("entity")
            .ok_or("world.insert_components needs an entity")?,
    )?;
    let components = params
        .get("components")
        .and_then(Value::as_object)
        .ok_or("world.insert_components needs a components object of name: value")?;
    apply_components(world, entity, components)?;
    Ok(json!({ "entity": entity_text(entity), "written": components.keys().collect::<Vec<_>>() }))
}

/// Lays `patch` over `base`: objects merge key by key, anything else replaces.
fn merge_patch(base: &mut Value, patch: &Value) {
    match (base, patch) {
        (Value::Object(base), Value::Object(patch)) => {
            for (key, value) in patch {
                match base.get_mut(key) {
                    Some(existing) => merge_patch(existing, value),
                    None => {
                        base.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (base, patch) => *base = patch.clone(),
    }
}

/// Checks every component first, then writes them all, so a bad one leaves the entity as it was.
fn apply_components(
    world: &mut World,
    entity: Entity,
    components: &Map<String, Value>,
) -> Result<(), String> {
    let app = registry_arc(world)?;
    let registry = app.read();
    let mut ready: Vec<(ReflectComponent, Box<dyn PartialReflect>)> = Vec::new();
    for (name, value) in components {
        let registration = registration(&registry, name)?;
        let short = short_name(registration);
        if !WRITABLE.contains(&short) {
            return Err(format!(
                "{short} cannot be changed yet; the writable components are {}",
                WRITABLE.join(", ")
            ));
        }
        let component = registration
            .data::<ReflectComponent>()
            .ok_or_else(|| format!("{name:?} is not a component"))?
            .clone();
        // A component the entity already has is PATCHED: what the request names replaces what it has (objects merge field by field, a
        // list or a number is replaced whole), the rest stays. So `{"roughness": 0.1}` changes the roughness and nothing else. A component
        // the entity does not have yet must be given in full.
        let mut merged = Value::Null;
        if let Some(current) = component.reflect(world.entity(entity)) {
            merged = serde_json::to_value(TypedReflectSerializer::new(current, &registry))
                .map_err(|e| e.to_string())?;
            merge_patch(&mut merged, value);
        }
        let value = if merged.is_null() { value } else { &merged };
        let text = value.to_string();
        let mut deserializer = serde_json::Deserializer::from_str(&text);
        let value = TypedReflectDeserializer::new(registration, &registry)
            .deserialize(&mut deserializer)
            .map_err(|e| {
                format!(
                    "{short}: {e} (read the value back with world.get_components to see its shape)"
                )
            })?;
        ready.push((component, value));
    }
    let mut entity_mut = world
        .get_entity_mut(entity)
        .map_err(|_| "the entity no longer exists".to_owned())?;
    for (component, value) in &ready {
        if component.contains(&entity_mut) {
            component.apply(&mut entity_mut, &**value);
        } else {
            component.insert(&mut entity_mut, &**value, &registry);
        }
    }
    Ok(())
}

fn query(world: &mut World, params: &Value) -> Result<Value, String> {
    let select = string_list(params.get("components"), "components")?;
    let has = string_list(params.get("has"), "has")?;
    let name_filter = params
        .get("name")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let limit = params
        .get("limit")
        .and_then(Value::as_u64)
        .map_or(MAX_QUERY_ROWS, |n| {
            usize::try_from(n)
                .unwrap_or(MAX_QUERY_ROWS)
                .min(MAX_QUERY_ROWS)
        });
    let app = registry_arc(world)?;
    let registry = app.read();
    let mut wanted: Vec<(String, ReflectComponent, TypeId)> = Vec::new();
    for name in select.iter().chain(&has) {
        let registration = registration(&registry, name)?;
        let component = registration
            .data::<ReflectComponent>()
            .ok_or_else(|| format!("{name:?} is not a component"))?
            .clone();
        wanted.push((name.clone(), component, registration.type_id()));
    }
    let mut rows = Vec::new();
    let ids: Vec<Entity> = world.iter_entities().map(|e| e.id()).collect();
    for id in ids {
        if rows.len() >= limit {
            break;
        }
        let entity_ref = world.entity(id);
        if !wanted
            .iter()
            .all(|(_, component, _)| component.contains(entity_ref))
        {
            continue;
        }
        let authored = world
            .get::<Authored>(id)
            .map(|a| (a.name().to_owned(), a.node().to_owned()));
        if let Some(filter) = &name_filter {
            if authored.as_ref().map(|(n, _)| n) != Some(filter) {
                continue;
            }
        }
        let mut values = Map::new();
        for (name, component, _) in wanted.iter().take(select.len()) {
            if let Some(value) = component.reflect(world.entity(id)) {
                if let Ok(json) =
                    serde_json::to_value(TypedReflectSerializer::new(value, &registry))
                {
                    values.insert(name.clone(), json);
                }
            }
        }
        let mut row = json!({ "entity": entity_text(id), "components": values });
        if let Some((name, node)) = authored {
            row["name"] = Value::String(name);
            row["node"] = Value::String(node);
        }
        rows.push(row);
    }
    Ok(json!({ "rows": rows, "truncated": rows.len() >= limit }))
}

/// Marks an entity made by `world.spawn_entity`, so `world.despawn_entity` may remove it (it never removes the engine's own entities).
#[derive(bevy_ecs::component::Component)]
struct ApiSpawned;

include!("world_api_methods.rs");

/// `world.spawn_entity {components, shape, position}`: an entity with the given components, and optionally a visible `shape` (see
/// `build_shape`) at `position` [x, y, z] (or whatever Transform `components` gives it).
fn spawn_entity(world: &mut World, params: &Value) -> Result<Value, String> {
    let empty = Map::new();
    let components = params
        .get("components")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    let alive = world.query::<&ApiSpawned>().iter(world).count();
    if alive >= MAX_API_ENTITIES {
        return Err(format!(
            "{alive} spawned entities are alive; despawn some first (the limit is {MAX_API_ENTITIES})"
        ));
    }
    let shape = match params.get("shape") {
        Some(shape) => Some(build_shape(world, shape)?),
        None => None,
    };
    let entity = world.spawn(ApiSpawned).id();
    if let Some((mesh, material)) = shape {
        let at = match params.get("position") {
            Some(_) => {
                let p = number_list(params.get("position"), 3, "position")?;
                #[expect(clippy::cast_possible_truncation, reason = "a position in metres")]
                bevy_math::Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32)
            }
            None => bevy_math::Vec3::ZERO,
        };
        world.entity_mut(entity).insert((
            mesh,
            material,
            bevy_transform::components::Transform::from_translation(at),
        ));
    }
    if let Err(error) = apply_components(world, entity, components) {
        world.despawn(entity);
        return Err(error);
    }
    Ok(json!({ "entity": entity_text(entity) }))
}

fn despawn_entity(world: &mut World, params: &Value) -> Result<Value, String> {
    let entity = entity_of(
        world,
        params
            .get("entity")
            .ok_or("world.despawn_entity needs an entity")?,
    )?;
    if world.get::<Authored>(entity).is_none() && world.get::<ApiSpawned>(entity).is_none() {
        return Err(
            "only authored parts and entities spawned through this API can be despawned".to_owned(),
        );
    }
    world.despawn(entity);
    Ok(json!({ "despawned": entity_text(entity) }))
}

fn string_list(value: Option<&Value>, what: &str) -> Result<Vec<String>, String> {
    match value {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| format!("{what} must be a list of strings"))
            })
            .collect(),
        Some(_) => Err(format!("{what} must be a list of strings")),
    }
}
