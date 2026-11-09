//! Party combat in the world: brains (entities that follow a leader and fight on their own), the leader's target, abilities with cooldowns,
//! and the actions `target:next|prev`, `dash`, `iframes` and `blast`. See `parse/party.rs` for the declarations.
use super::combat::ticks;
use super::{Entity, Event, World, matches_selector};
use crate::{Effect, GameError, Inputs};

const DT: f64 = 1.0 / 60.0;

/// An entity id kept in a prop (props are numbers) as an id again.
fn as_id(value: f64) -> u32 {
    format!("{:.0}", value.max(0.0)).parse().unwrap_or(u32::MAX)
}

fn flat_distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] - b[0]).hypot(a[2] - b[2])
}

impl World {
    /// Whether a body of this radius standing at (x, z) would be inside a `solid` box (at knee height).
    fn blocked_at(&self, x: f64, z: f64, radius: f64) -> bool {
        self.entities.iter().any(|e| {
            let Some(extents) = self.def.kinds.iter().find(|k| k.name == e.kind).and_then(|k| k.solid) else { return false };
            (x - e.position[0]).abs() < extents[0] / 2.0 + radius
                && (z - e.position[2]).abs() < extents[2] / 2.0 + radius
                && (e.position[1] - extents[1] / 2.0..=e.position[1] + extents[1] / 2.0).contains(&0.5)
        })
    }

    /// Moves an entity toward a point at `speed` (sliding along walls), turning it to face where it is going.
    fn step_toward(&mut self, id: u32, goal: [f64; 3], speed: f64) {
        let Some(entity) = self.entities.iter().find(|e| e.id == id) else { return };
        let (position, radius) = (entity.position, self.def.kinds.iter().find(|k| k.name == entity.kind).map_or(0.4, |k| k.size));
        let (dx, dz) = (goal[0] - position[0], goal[2] - position[2]);
        let distance = dx.hypot(dz);
        if distance < 1e-6 {
            return;
        }
        let stride = (speed * DT).min(distance);
        let (nx, nz) = (position[0] + dx / distance * stride, position[2] + dz / distance * stride);
        let target = if !self.blocked_at(nx, nz, radius) {
            Some((nx, nz))
        } else if !self.blocked_at(nx, position[2], radius) {
            Some((nx, position[2]))
        } else if !self.blocked_at(position[0], nz, radius) {
            Some((position[0], nz))
        } else {
            None
        };
        if let Some(entity) = self.entities.iter_mut().find(|e| e.id == id) {
            entity.heading = dx.atan2(dz);
            if let Some((x, z)) = target {
                entity.position[0] = x;
                entity.position[2] = z;
            }
        }
    }

    fn face(&mut self, id: u32, toward: [f64; 3]) {
        if let Some(entity) = self.entities.iter_mut().find(|e| e.id == id) {
            entity.heading = (toward[0] - entity.position[0]).atan2(toward[2] - entity.position[2]);
        }
    }

    /// The nearest living foe of an entity within `reach` metres, as (id, position).
    fn nearest_foe(&self, from: &Entity, foes: &str, reach: f64) -> Option<(u32, [f64; 3])> {
        self.entities
            .iter()
            .filter(|e| e.id != from.id && self.is_victim(e) && matches_selector(&self.def, foes, e))
            .map(|e| (e.id, e.position, flat_distance(e.position, from.position)))
            .filter(|(_, _, d)| *d <= reach)
            .min_by(|a, b| a.2.total_cmp(&b.2))
            .map(|(id, position, _)| (id, position))
    }

    /// Moves and fights every brain-driven entity one step.
    pub(super) fn advance_brains(&mut self, queue: &mut Vec<Event>) {
        for brain in self.def.brains.clone() {
            let ids: Vec<u32> = self.entities.iter().filter(|e| e.kind == brain.kind && e.props.get("health").is_none_or(|h| *h > 0.0)).map(|e| e.id).collect();
            let weapon = brain.weapon.as_ref().and_then(|name| self.def.weapons.iter().position(|w| w.name == *name));
            let reach = match weapon.map(|w| &self.def.weapons[w]) {
                Some(w) if w.mode == "melee" => w.range * 0.8,
                _ => brain.keep.unwrap_or(6.0),
            };
            for id in ids {
                let Some(me) = self.entities.iter().find(|e| e.id == id).cloned() else { continue };
                if let Some((_, foe)) = self.nearest_foe(&me, &brain.foes, brain.engage) {
                    if flat_distance(foe, me.position) > reach {
                        self.step_toward(id, foe, brain.speed);
                    } else {
                        self.face(id, foe);
                    }
                    if let Some(weapon) = weapon
                        && flat_distance(foe, me.position) <= reach + 1.0
                    {
                        self.fire_weapon(weapon, id, queue);
                    }
                } else if let Some(leader) = &brain.leader {
                    let near = self
                        .entities
                        .iter()
                        .filter(|e| e.id != id && matches_selector(&self.def, leader, e))
                        .map(|e| e.position)
                        .min_by(|a, b| flat_distance(*a, me.position).total_cmp(&flat_distance(*b, me.position)));
                    if let Some(position) = near {
                        let gap = flat_distance(position, me.position);
                        if gap > brain.follow {
                            self.step_toward(id, position, brain.speed * if gap > brain.follow * 3.0 { 1.4 } else { 1.0 });
                        }
                    }
                }
                if let Some(entity) = self.entities.iter_mut().find(|e| e.id == id) {
                    let half = entity.heading / 2.0;
                    entity.orientation = [0.0, half.sin(), 0.0, half.cos()];
                }
            }
        }
    }

    /// The foes the leader could target, nearest first: (id, distance).
    fn target_candidates(&self, leader: &Entity, foes: &str, range: f64) -> Vec<(u32, f64)> {
        let mut found: Vec<(u32, f64)> = self
            .entities
            .iter()
            .filter(|e| e.id != leader.id && self.is_victim(e) && matches_selector(&self.def, foes, e))
            .map(|e| (e.id, flat_distance(e.position, leader.position)))
            .filter(|(_, d)| *d <= range)
            .collect();
        found.sort_by(|a, b| a.1.total_cmp(&b.1));
        found
    }

    fn set_target(&mut self, leader: u32, target: Option<u32>, foes: &str) {
        let flagged: Vec<u32> = self.entities.iter().filter(|e| matches_selector(&self.def, foes, e)).map(|e| e.id).collect();
        for entity in &mut self.entities {
            if entity.id == leader {
                entity.props.insert("target".into(), target.map_or(-1.0, f64::from));
            } else if flagged.contains(&entity.id) {
                let now = if Some(entity.id) == target { 1.0 } else { 0.0 };
                if now > 0.0 || entity.props.get("targeted").is_some_and(|v| *v > 0.0) {
                    entity.props.insert("targeted".into(), now);
                }
            }
        }
    }

    /// Keeps the leader's target: the one it already has while that foe lives and is in range, else the nearest foe in front of it (or the
    /// nearest of all when none is in front).
    pub(super) fn update_targeting(&mut self, input: &Inputs) {
        let Some(targeting) = self.def.targeting.clone() else { return };
        let Some(leader) = self.entities.iter().find(|e| matches_selector(&self.def, &targeting.leader, e)).cloned() else { return };
        let candidates = self.target_candidates(&leader, &targeting.foes, targeting.range);
        let current = leader.props.get("target").copied().filter(|t| *t >= 0.0);
        if current.is_some_and(|t| candidates.iter().any(|(id, _)| *id == as_id(t))) {
            return;
        }
        let facing = input.axes.get("camera.yaw").copied().unwrap_or(leader.heading);
        let half_cone = (targeting.cone / 2.0).to_radians();
        let ahead = |id: u32| {
            self.entities.iter().find(|e| e.id == id).is_some_and(|e| {
                let bearing = (e.position[0] - leader.position[0]).atan2(e.position[2] - leader.position[2]);
                (bearing - facing + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
            }.abs() <= half_cone)
        };
        let chosen = candidates.iter().find(|(id, _)| ahead(*id)).or(candidates.first()).map(|(id, _)| *id);
        self.set_target(leader.id, chosen, &targeting.foes);
    }

    fn cycle_target(&mut self, step: isize) {
        let Some(targeting) = self.def.targeting.clone() else { return };
        let Some(leader) = self.entities.iter().find(|e| matches_selector(&self.def, &targeting.leader, e)).cloned() else { return };
        let candidates = self.target_candidates(&leader, &targeting.foes, targeting.range);
        if candidates.is_empty() {
            return;
        }
        let current = leader.props.get("target").copied().filter(|t| *t >= 0.0).map(as_id);
        let at = current.and_then(|current| candidates.iter().position(|(id, _)| *id == current));
        let count = isize::try_from(candidates.len()).unwrap_or(1);
        let next = at.map_or(0, |i| usize::try_from((isize::try_from(i).unwrap_or(0) + step).rem_euclid(count)).unwrap_or(0));
        self.set_target(leader.id, Some(candidates[next].0), &targeting.foes);
    }

    /// Counts the abilities' cooldowns down and runs the ones whose input was pressed.
    pub(super) fn ability_step(&mut self, input: &Inputs, queue: &mut Vec<Event>, effects: &mut Vec<Effect>) -> Result<(), GameError> {
        for ability in self.def.abilities.clone() {
            let owners: Vec<u32> = self.entities.iter().filter(|e| matches_selector(&self.def, &ability.owner, e)).map(|e| e.id).collect();
            let (cd, cdmax) = (format!("cd_{}", ability.name), format!("cdmax_{}", ability.name));
            for owner in owners {
                if let Some(entity) = self.entities.iter_mut().find(|e| e.id == owner) {
                    let left = entity.props.get(&cd).copied().unwrap_or(0.0);
                    entity.props.insert(cd.clone(), (left - DT).max(0.0));
                    entity.props.insert(cdmax.clone(), ability.cooldown);
                }
                if input.pressed.contains(&ability.input) {
                    self.use_ability(&ability, owner, queue, effects)?;
                }
            }
        }
        Ok(())
    }

    fn use_ability(&mut self, ability: &crate::AbilityDef, owner: u32, queue: &mut Vec<Event>, effects: &mut Vec<Effect>) -> Result<(), GameError> {
        let Some(me) = self.entities.iter().find(|e| e.id == owner).cloned() else { return Ok(()) };
        let refused = Event { name: format!("refused:{}", ability.name), a: Some(owner), b: None, depth: 1 };
        let ready = me.props.get(&format!("cd_{}", ability.name)).copied().unwrap_or(0.0) <= 0.0;
        let paid = ability.cost <= 0.0 || me.props.get(&ability.cost_prop).copied().unwrap_or(0.0) >= ability.cost;
        let target = me.props.get("target").copied().filter(|t| *t >= 0.0).and_then(|t| self.entities.iter().find(|e| e.id == as_id(t) && self.is_victim(e)));
        let in_reach = target.is_some_and(|t| ability.range <= 0.0 || flat_distance(t.position, me.position) <= ability.range);
        if !ready || !paid || (ability.needs_target && !in_reach) {
            queue.push(refused);
            return Ok(());
        }
        let target_id = target.map(|t| t.id);
        if let Some(entity) = self.entities.iter_mut().find(|e| e.id == owner) {
            if ability.cost > 0.0 {
                *entity.props.entry(ability.cost_prop.clone()).or_insert(0.0) -= ability.cost;
            }
            entity.props.insert(format!("cd_{}", ability.name), ability.cooldown);
        }
        let event = Event { name: format!("ability:{}", ability.name), a: Some(owner), b: target_id, depth: 1 };
        queue.push(event.clone());
        for action in &ability.actions {
            self.action(action, &event, queue, effects)?;
        }
        Ok(())
    }

    /// `target:next|prev`, `dash`, `iframes` and `blast`; returns whether `action` was one of them.
    pub(super) fn party_action(&mut self, action: &str, event: &Event, queue: &mut Vec<Event>) -> bool {
        if action == "target:next" || action == "target:prev" {
            self.cycle_target(if action == "target:next" { 1 } else { -1 });
        } else if let Some(body) = action.strip_prefix("dash:") {
            let Some((rest, seconds)) = body.rsplit_once(':') else { return true };
            let Some((selector, speed)) = rest.rsplit_once(':') else { return true };
            let (speed, seconds) = (speed.parse::<f64>().unwrap_or(0.0), seconds.parse::<f64>().unwrap_or(0.0));
            for id in self.select_ids(selector, event) {
                if let Some(entity) = self.entities.iter_mut().find(|e| e.id == id) {
                    let moving = entity.velocity[0].hypot(entity.velocity[2]) > 0.5;
                    let (dx, dz) = if moving { (entity.velocity[0], entity.velocity[2]) } else { (entity.heading.sin(), entity.heading.cos()) };
                    let length = dx.hypot(dz).max(1e-9);
                    entity.props.insert("dash_left".into(), f64::from(ticks(seconds)));
                    entity.props.insert("dash_vx".into(), dx / length * speed);
                    entity.props.insert("dash_vz".into(), dz / length * speed);
                }
            }
        } else if let Some(body) = action.strip_prefix("iframes:") {
            let Some((selector, seconds)) = body.rsplit_once(':') else { return true };
            let seconds = seconds.parse::<f64>().unwrap_or(0.0);
            for id in self.select_ids(selector, event) {
                if let Some(entity) = self.entities.iter_mut().find(|e| e.id == id) {
                    entity.props.insert("iframes_left".into(), f64::from(ticks(seconds)));
                }
            }
        } else if let Some(body) = action.strip_prefix("blast:") {
            let (spec, foes) = body.split_once('@').map_or((body, ""), |(spec, foes)| (spec, foes));
            let Some((rest, amount)) = spec.rsplit_once(':') else { return true };
            let Some((centre, radius)) = rest.rsplit_once(':') else { return true };
            let (amount, radius) = (amount.parse::<f64>().unwrap_or(0.0), radius.parse::<f64>().unwrap_or(0.0));
            let Some(source) = self.select_ids(centre, event).first().copied() else { return true };
            let Some(origin) = self.entities.iter().find(|e| e.id == source).map(|e| [e.position[0], e.position[1] + 0.5, e.position[2]]) else { return true };
            let victims: Vec<u32> = self
                .entities
                .iter()
                .filter(|e| e.id != source && self.is_victim(e) && (foes.is_empty() || matches_selector(&self.def, foes, e)))
                .filter(|e| self.distance_to_body(e, origin) <= radius)
                .map(|e| e.id)
                .collect();
            for victim in victims {
                self.apply_damage(victim, amount, Some(source), queue);
            }
        } else {
            return false;
        }
        true
    }
}
