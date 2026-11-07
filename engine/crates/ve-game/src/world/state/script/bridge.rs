//! What a script can do to the world while a handler runs.
//!
//! The world lends its entities and variables to the bridge for the length of one dispatch, so a script reads what it wrote a moment
//! ago and nothing needs unsafe code or a second copy. Everything else a script asks for (events, effects, spawns) is recorded here and
//! applied by the world afterwards, in order.
use super::super::{Entity, MAX_ENTITIES, matches_selector, next_random};
use crate::{Effect, EffectKind, GameDef, Status};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub(super) struct Bridge {
    pub(super) def: GameDef,
    pub(super) entities: Vec<Entity>,
    pub(super) vars: BTreeMap<String, f64>,
    pub(super) rng: u64,
    pub(super) next_id: u32,
    pub(super) status: Status,
    pub(super) raised: Vec<(String, Option<u32>, Option<u32>)>,
    pub(super) effects: Vec<Effect>,
    pub(super) destroyed: Vec<u32>,
    pub(super) spawned: Vec<u32>,
    pub(super) changed: Vec<String>,
    pub(super) log: Vec<String>,
    /// The game UI scripts have set, in the order they were first set (drawing order).
    pub(super) ui: Vec<(String, Value)>,
    /// The host's input for the step in progress.
    pub(super) axes: BTreeMap<String, f64>,
    pub(super) presses: Vec<String>,
    /// Where scripts want the view: eye position and target, kept until they change it.
    pub(super) camera: Option<([f64; 3], [f64; 3])>,
    /// A script asked for the game to start over.
    pub(super) reset: bool,
}

fn id_of(args: &Value, key: &str) -> Option<u32> {
    args.get(key)
        .and_then(Value::as_i64)
        .and_then(|value| u32::try_from(value).ok())
}

fn vec3(value: Option<&Value>) -> Option<[f64; 3]> {
    let items = value?.as_array()?;
    if items.len() != 3 {
        return None;
    }
    let mut out = [0.0; 3];
    for (slot, item) in out.iter_mut().zip(items) {
        *slot = item.as_f64().filter(|number| number.is_finite())?;
    }
    Some(out)
}

fn finite(value: Option<&Value>) -> Option<f64> {
    value
        .and_then(Value::as_f64)
        .filter(|number| number.is_finite())
}

impl Bridge {
    fn entity(&self, id: u32) -> Option<&Entity> {
        self.entities.iter().find(|entity| entity.id == id)
    }

    fn entity_mut(&mut self, id: u32) -> Result<&mut Entity, String> {
        self.entities
            .iter_mut()
            .find(|entity| entity.id == id)
            .ok_or_else(|| format!("entity {id} does not exist"))
    }

    fn view(&self, entity: &Entity) -> Value {
        let kind = self.def.kinds.iter().find(|kind| kind.name == entity.kind);
        let tags = kind.map(|kind| kind.tags.clone()).unwrap_or_default();
        json!({
            "id": entity.id, "kind": entity.kind, "pos": entity.position, "vel": entity.velocity,
            "heading": entity.heading, "props": entity.props, "tags": tags,
            "size": kind.map_or(0.5, |kind| kind.size),
        })
    }

    fn need_id(args: &Value) -> Result<u32, String> {
        id_of(args, "id").ok_or_else(|| "an entity id is needed".to_owned())
    }

    /// One call from a script: an operation name and its JSON arguments, answered with JSON or a message the script sees as an error.
    pub(super) fn call(&mut self, op: &str, args: &Value) -> Result<Value, String> {
        match op {
            "ent" => Ok(self
                .entity(Self::need_id(args)?)
                .map_or(Value::Null, |entity| self.view(entity))),
            "set" => self.set_field(args),
            "prop" => self.property(args),
            "find" => {
                let selector = args.get("s").and_then(Value::as_str).unwrap_or("");
                Ok(json!(
                    self.entities
                        .iter()
                        .filter(|entity| matches_selector(&self.def, selector, entity))
                        .map(|entity| entity.id)
                        .collect::<Vec<_>>()
                ))
            }
            "sel" => {
                let selector = args.get("s").and_then(Value::as_str).unwrap_or("");
                Ok(json!(self.entity(Self::need_id(args)?).is_some_and(
                    |entity| matches_selector(&self.def, selector, entity)
                )))
            }
            "spawn" => self.spawn(args),
            "destroy" => {
                let id = Self::need_id(args)?;
                if self.entity(id).is_some() {
                    self.entities.retain(|entity| entity.id != id);
                    self.destroyed.push(id);
                }
                Ok(Value::Null)
            }
            "emit" => {
                let name = args.get("name").and_then(Value::as_str).unwrap_or("");
                if name.is_empty() {
                    return Err("emit needs an event name".into());
                }
                self.raised
                    .push((name.to_owned(), id_of(args, "a"), id_of(args, "b")));
                Ok(Value::Null)
            }
            "var" => self.variable(args),
            "fx" => self.effect(args),
            "end" => {
                self.status = if args.get("win").and_then(Value::as_bool) == Some(true) {
                    Status::Won
                } else {
                    Status::Lost
                };
                Ok(Value::Null)
            }
            "log" => {
                let text = args.get("text").and_then(Value::as_str).unwrap_or("");
                self.log.push(text.chars().take(500).collect());
                Ok(Value::Null)
            }
            "err" => {
                let field = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or("");
                self.log.push(format!(
                    "error: script '{}' while handling '{}': {}",
                    field("script"),
                    field("event"),
                    field("message")
                ));
                Ok(Value::Null)
            }
            "rand" => Ok(json!(next_random(&mut self.rng))),
            "ui_set" => self.ui_set(args),
            "ui_remove" => {
                let id = args.get("id").and_then(Value::as_str).unwrap_or("");
                self.ui.retain(|(name, _)| name != id);
                Ok(Value::Null)
            }
            "ui_clear" => {
                self.ui.clear();
                Ok(Value::Null)
            }
            "ui_measure" => Ok(json!(crate::ui::measure(args))),
            "inp" => {
                let name = args.get("name").and_then(Value::as_str).unwrap_or("");
                Ok(json!(self.axes.get(name).copied().unwrap_or(0.0)))
            }
            "inpp" => {
                let name = args.get("name").and_then(Value::as_str).unwrap_or("");
                Ok(json!(self.presses.iter().any(|pressed| pressed == name)))
            }
            "reset" => {
                self.reset = true;
                Ok(Value::Null)
            }
            "cam" => {
                let pos = vec3(args.get("pos")).ok_or("camera needs pos [x, y, z]")?;
                let target = vec3(args.get("target")).ok_or("camera needs target [x, y, z]")?;
                self.camera = Some((pos, target));
                Ok(Value::Null)
            }
            other => Err(format!("unknown host call '{other}'")),
        }
    }

    fn ui_set(&mut self, args: &Value) -> Result<Value, String> {
        let id = args.get("id").and_then(Value::as_str).unwrap_or("");
        if id.is_empty() || id.len() > 64 {
            return Err("a UI element needs an id of 1 to 64 characters".into());
        }
        let spec = args.get("spec").cloned().unwrap_or(Value::Null);
        if !crate::ui::known(&spec) {
            return Err("a UI element needs t: 'text', 'bar' or 'panel'".into());
        }
        if let Some(slot) = self.ui.iter_mut().find(|(name, _)| name == id) {
            slot.1 = spec;
        } else if self.ui.len() >= crate::ui::MAX_ELEMENTS {
            return Err(format!(
                "at most {} UI elements at once",
                crate::ui::MAX_ELEMENTS
            ));
        } else {
            self.ui.push((id.to_owned(), spec));
        }
        Ok(Value::Null)
    }

    fn set_field(&mut self, args: &Value) -> Result<Value, String> {
        let id = Self::need_id(args)?;
        let field = args.get("f").and_then(Value::as_str).unwrap_or("");
        let entity = self.entity_mut(id)?;
        match field {
            "pos" => {
                entity.position =
                    vec3(args.get("v")).ok_or("pos needs [x, y, z] of finite numbers")?
            }
            "vel" => {
                entity.velocity =
                    vec3(args.get("v")).ok_or("vel needs [x, y, z] of finite numbers")?
            }
            "heading" => {
                entity.heading =
                    finite(args.get("v")).ok_or("heading needs a finite number of radians")?
            }
            other => return Err(format!("cannot set '{other}'")),
        }
        Ok(Value::Null)
    }

    fn property(&mut self, args: &Value) -> Result<Value, String> {
        let id = Self::need_id(args)?;
        let name = args
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let entity = self.entity_mut(id)?;
        if let Some(value) = args.get("v") {
            let value = finite(Some(value)).ok_or("a property is a finite number")?;
            entity.props.insert(name, value);
            return Ok(json!(value));
        }
        if let Some(delta) = args.get("add") {
            let delta = finite(Some(delta)).ok_or("a property is a finite number")?;
            let slot = entity.props.entry(name).or_insert(0.0);
            *slot += delta;
            return Ok(json!(*slot));
        }
        Ok(entity
            .props
            .get(&name)
            .map_or(Value::Null, |value| json!(value)))
    }

    fn variable(&mut self, args: &Value) -> Result<Value, String> {
        let name = args
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let old = self.vars.get(&name).copied();
        let new = if let Some(value) = args.get("v") {
            Some(finite(Some(value)).ok_or("a variable is a finite number")?)
        } else if let Some(delta) = args.get("add") {
            Some(old.unwrap_or(0.0) + finite(Some(delta)).ok_or("a variable is a finite number")?)
        } else {
            None
        };
        let Some(new) = new else {
            return Ok(old.map_or(Value::Null, |value| json!(value)));
        };
        self.vars.insert(name.clone(), new);
        if old.is_none_or(|old| old.to_bits() != new.to_bits()) {
            self.changed.push(name);
        }
        Ok(json!(new))
    }

    fn spawn(&mut self, args: &Value) -> Result<Value, String> {
        let kind_name = args.get("kind").and_then(Value::as_str).unwrap_or("");
        let kind = self
            .def
            .kinds
            .iter()
            .find(|kind| kind.name == kind_name)
            .ok_or_else(|| format!("unknown kind '{kind_name}'"))?;
        if self.entities.len() >= MAX_ENTITIES {
            return Err(format!("entity budget exceeded ({MAX_ENTITIES})"));
        }
        let position = vec3(args.get("pos")).ok_or("spawn needs a pos of [x, y, z]")?;
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.entities.push(Entity {
            id,
            kind: kind_name.to_owned(),
            position,
            heading: finite(args.get("heading")).unwrap_or(0.0),
            orientation: [0.0, 0.0, 0.0, 1.0],
            velocity: [0.0; 3],
            props: kind.props.clone(),
        });
        self.spawned.push(id);
        Ok(json!(id))
    }

    fn effect(&mut self, args: &Value) -> Result<Value, String> {
        let kind = match args.get("kind").and_then(Value::as_str).unwrap_or("") {
            "sfx" => EffectKind::Sfx,
            "particles" => EffectKind::Particles,
            "shake" => EffectKind::CameraShake,
            "message" => EffectKind::Message,
            "anim" => EffectKind::Anim,
            "anim_set" => EffectKind::AnimSet,
            "anim_flag" => EffectKind::AnimFlag,
            other => {
                return Err(format!(
                    "unknown effect '{other}': use sfx, particles, shake, message, anim, anim_set or anim_flag"
                ));
            }
        };
        self.effects.push(Effect {
            kind,
            name: args
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned(),
            target: id_of(args, "target"),
            value: finite(args.get("value")),
        });
        Ok(Value::Null)
    }
}
