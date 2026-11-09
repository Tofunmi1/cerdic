//! Health and damage (F2): the `hurt` and `heal` actions, invulnerability frames, knockback, and the events `hurt` and `killed`.
//!
//! An entity of a kind with a `health` statement carries the props `health` and `max_health`. A hurt below zero raises `killed` and the
//! entity is removed after the rules have heard about it (the event `reap:ID`, handled by the event loop), so a rule on `killed a=tag:enemy`
//! still finds the entity it is about. The weapons and shots that deal damage are in `ranged.rs`.
use std::collections::BTreeMap;

use super::{Entity, Event, World};
use crate::HealthDef;
use crate::ui::layout::{FLOATER_LIFE, Floater};

/// One weapon's magazine and timers for one owner.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct WeaponState {
    pub(super) ammo: u32,
    pub(super) cooldown: u32,
    pub(super) reloading: u32,
}

/// The combat state kept in saves and the hash: every weapon's state, keyed `OWNER_ID:WEAPON`.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct CombatState {
    pub(super) weapons: BTreeMap<String, WeaponState>,
}

const DT_STEPS: f64 = 60.0;

/// Seconds as whole fixed steps (at least 1 for anything above zero).
pub(super) fn ticks(seconds: f64) -> u32 {
    if seconds <= 0.0 {
        return 0;
    }
    format!("{:.0}", (seconds * DT_STEPS).round().clamp(1.0, 1.0e9)).parse().unwrap_or(1)
}

fn queue_event(queue: &mut Vec<Event>, name: &str, a: Option<u32>, b: Option<u32>) {
    queue.push(Event { name: name.to_owned(), a, b, depth: 1 });
}

impl World {
    pub(super) fn health_def(&self, kind: &str) -> Option<&HealthDef> {
        self.def.healths.iter().find(|health| health.kind == kind)
    }

    /// How tall an entity's hit shape is: the kind's `height`, else twice its size.
    pub(super) fn hit_height(&self, entity: &Entity) -> f64 {
        let size = self.def.kinds.iter().find(|kind| kind.name == entity.kind).map_or(0.5, |kind| kind.size);
        self.health_def(&entity.kind).and_then(|health| health.height).unwrap_or(2.0 * size)
    }

    /// The distance from a point to the vertical line segment from an entity's feet to the top of its hit shape.
    pub(super) fn distance_to_body(&self, entity: &Entity, point: [f64; 3]) -> f64 {
        let top = entity.position[1] + self.hit_height(entity);
        let y = point[1].clamp(entity.position[1], top);
        ((point[0] - entity.position[0]).powi(2) + (point[1] - y).powi(2) + (point[2] - entity.position[2]).powi(2)).sqrt()
    }

    /// Whether an entity can be hurt right now: it has health left and is not a shot in flight.
    pub(super) fn is_victim(&self, entity: &Entity) -> bool {
        entity.props.get("health").is_some_and(|health| *health > 0.0) && !self.def.projectiles.iter().any(|shot| shot.kind == entity.kind)
    }

    /// Starts a floating number over an entity's head (`kind`: 0 damage, 1 healing, 2 dodge).
    fn float_over(&mut self, entity: u32, text: String, kind: u8) {
        let Some(found) = self.entities.iter().find(|e| e.id == entity) else { return };
        let pos = [found.position[0], found.position[1] + self.hit_height(found), found.position[2]];
        self.floaters.push(Floater { pos, text, kind, age: 0 });
        if self.floaters.len() > 32 {
            self.floaters.remove(0);
        }
    }

    /// Counts down the invulnerability frames of everything that has them, and ages the floating numbers.
    pub(super) fn tick_combat(&mut self) {
        self.floaters.retain_mut(|floater| {
            floater.age += 1;
            floater.age < FLOATER_LIFE
        });
        if self.def.healths.is_empty() {
            return;
        }
        let pushable: Vec<String> = self.def.healths.iter().filter(|h| h.knock > 0.0).map(|h| h.kind.clone()).collect();
        let controlled: Vec<String> = self.def.controllers.iter().map(|c| c.kind.clone()).collect();
        for entity in &mut self.entities {
            if let Some(left) = entity.props.get_mut("iframes_left")
                && *left > 0.0
            {
                *left -= 1.0;
            }
            // a knockback is a shove that friction wears off (a controlled kind slows itself with its own deceleration)
            if pushable.contains(&entity.kind) && !controlled.contains(&entity.kind) {
                for axis in [0, 2] {
                    entity.velocity[axis] *= 0.88;
                    if entity.velocity[axis].abs() < 0.02 {
                        entity.velocity[axis] = 0.0;
                    }
                }
            }
        }
    }

    /// Gives a new entity the health its kind declares.
    pub(super) fn give_health(&mut self, kind: &str, props: &mut BTreeMap<String, f64>) {
        if let Some(health) = self.health_def(kind) {
            props.insert("health".into(), health.hp);
            props.insert("max_health".into(), health.hp);
        }
    }

    /// Puts the full magazine of every weapon an entity carries on it as the prop `ammo_NAME`, so a HUD can show it before the first shot.
    pub(super) fn give_ammo(&mut self, id: u32) {
        let Some(entity) = self.entities.iter().find(|entity| entity.id == id) else { return };
        let full: Vec<(String, u32)> = self
            .def
            .weapons
            .iter()
            .filter(|weapon| weapon.ammo > 0 && super::matches_selector(&self.def, &weapon.owner, entity))
            .map(|weapon| (format!("ammo_{}", weapon.name), weapon.ammo))
            .collect();
        if let Some(entity) = self.entities.iter_mut().find(|entity| entity.id == id) {
            for (name, ammo) in full {
                entity.props.insert(name, f64::from(ammo));
            }
        }
    }

    /// Takes `amount` from an entity's health unless it is untouchable; raises `hurt`, and `killed` at zero. Returns whether it was hurt.
    pub(super) fn apply_damage(&mut self, victim: u32, amount: f64, source: Option<u32>, queue: &mut Vec<Event>) -> bool {
        let Some(index) = self.entities.iter().position(|entity| entity.id == victim) else { return false };
        let Some(health) = self.health_def(&self.entities[index].kind).cloned() else { return false };
        let from = source.and_then(|id| self.entities.iter().find(|entity| entity.id == id)).map(|entity| entity.position);
        let entity = &mut self.entities[index];
        let now = entity.props.get("health").copied().unwrap_or(0.0);
        if now <= 0.0 {
            return false;
        }
        if entity.props.get("iframes_left").copied().unwrap_or(0.0) > 0.0 {
            self.float_over(victim, "Dodge!".to_owned(), 2);
            return false;
        }
        let left = (now - amount).max(0.0);
        self.float_over(victim, format!("{:.0}", (now - left).ceil()), 0);
        let entity = &mut self.entities[index];
        entity.props.insert("health".into(), left);
        if health.iframes > 0.0 {
            entity.props.insert("iframes_left".into(), f64::from(ticks(health.iframes)));
        }
        if health.knock > 0.0
            && let Some(from) = from
        {
            let (dx, dz) = (entity.position[0] - from[0], entity.position[2] - from[2]);
            let length = dx.hypot(dz);
            if length > 1e-9 {
                entity.velocity[0] += dx / length * health.knock;
                entity.velocity[2] += dz / length * health.knock;
            }
        }
        queue_event(queue, "hurt", Some(victim), source);
        if left <= 0.0 {
            queue_event(queue, "killed", Some(victim), source);
            queue_event(queue, &format!("reap:{victim}"), None, None);
        }
        true
    }

    fn heal(&mut self, target: u32, amount: f64) {
        let mut healed = 0.0;
        if let Some(entity) = self.entities.iter_mut().find(|entity| entity.id == target) {
            let max = entity.props.get("max_health").copied().unwrap_or(0.0);
            if let Some(health) = entity.props.get_mut("health")
                && *health > 0.0
            {
                let before = *health;
                *health = (*health + amount).min(max);
                healed = *health - before;
            }
        }
        if healed > 0.0 {
            self.float_over(target, format!("+{:.0}", healed.ceil()), 1);
        }
    }

    /// Runs `hurt`, `heal`, `fire` and `reload`; returns whether `action` was one of them.
    pub(super) fn combat_action(&mut self, action: &str, event: &Event, queue: &mut Vec<Event>) -> bool {
        if let Some(body) = action.strip_prefix("hurt:").or_else(|| action.strip_prefix("heal:")) {
            let (spec, source) = body.split_once('@').map_or((body, None), |(spec, source)| (spec, Some(source)));
            let Some((selector, amount)) = spec.rsplit_once(':') else { return true };
            let amount = amount.parse::<f64>().unwrap_or(0.0);
            let victims = self.select_ids(selector, event);
            let source_id = match source {
                Some(selector) => self.select_ids(selector, event).first().copied(),
                None => [event.a, event.b].into_iter().flatten().find(|id| !victims.contains(id)),
            };
            for victim in victims {
                if action.starts_with("hurt:") {
                    self.apply_damage(victim, amount, source_id, queue);
                } else {
                    self.heal(victim, amount);
                }
            }
            return true;
        }
        if let Some(body) = action.strip_prefix("fire:").or_else(|| action.strip_prefix("reload:")) {
            let (name, selector) = body.split_once('@').map_or((body, None), |(name, selector)| (name, Some(selector)));
            let Some(weapon) = self.def.weapons.iter().position(|weapon| weapon.name == name) else { return true };
            let owners = self.select_ids(selector.unwrap_or(&self.def.weapons[weapon].owner.clone()), event);
            for owner in owners {
                if action.starts_with("fire:") {
                    self.fire_weapon(weapon, owner, queue);
                } else {
                    self.start_reload(weapon, owner);
                }
            }
            return true;
        }
        false
    }
}
