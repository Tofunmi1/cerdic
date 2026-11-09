//! Weapons, shots and rays (F2). A `weapon` fires when its input is pressed (or held, with `auto`), or when the action `fire:NAME` runs;
//! it keeps a magazine (`ammo`, prop `ammo_NAME` on the owner), a cooldown and a reload. A projectile weapon spawns an entity of the shot
//! kind that flies each step (gravity, homing), hurts the first victim in its way, may pierce on, splash on impact, and ends on a wall or
//! when its life runs out; a hitscan weapon hurts the first victim along a ray at once. Walls are the game's `solid` boxes.
//!
//! Aim: `heading` (the way the owner faces), `camera` (through the crosshair: the host supplies `camera.yaw`, `camera.pitch` and
//! `camera.x/y/z`, and the shot goes from the muzzle to the first thing the camera ray meets) or a selector (toward the nearest such entity).
use super::combat::{WeaponState, ticks};
use super::{Entity, Event, World, matches_selector};
use crate::{Inputs, WeaponDef};

const DT: f64 = 1.0 / 60.0;
const FAR: f64 = 100.0;

/// An entity id kept in a prop (props are numbers) as an id again.
fn id_of(value: f64) -> u32 {
    format!("{:.0}", value.max(0.0)).parse().unwrap_or(u32::MAX)
}

fn forward(yaw: f64, pitch: f64) -> [f64; 3] {
    [pitch.cos() * yaw.sin(), pitch.sin(), pitch.cos() * yaw.cos()]
}

fn unit(v: [f64; 3]) -> [f64; 3] {
    let length = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if length < 1e-12 { [0.0, 0.0, 1.0] } else { [v[0] / length, v[1] / length, v[2] / length] }
}

fn at(origin: [f64; 3], dir: [f64; 3], t: f64) -> [f64; 3] {
    [origin[0] + dir[0] * t, origin[1] + dir[1] * t, origin[2] + dir[2] * t]
}

/// The distance along a ray (unit `dir`) to a sphere, if it meets it ahead.
fn ray_sphere(origin: [f64; 3], dir: [f64; 3], centre: [f64; 3], radius: f64) -> Option<f64> {
    let to = [centre[0] - origin[0], centre[1] - origin[1], centre[2] - origin[2]];
    let along = to[0] * dir[0] + to[1] * dir[1] + to[2] * dir[2];
    let off2 = to[0] * to[0] + to[1] * to[1] + to[2] * to[2] - along * along;
    let reach2 = radius * radius - off2;
    if reach2 < 0.0 {
        return None;
    }
    let t = along - reach2.sqrt();
    if t >= 0.0 { Some(t) } else { (along + reach2.sqrt() >= 0.0).then_some(0.0) }
}

impl World {
    fn kind_size(&self, kind: &str) -> f64 {
        self.def.kinds.iter().find(|k| k.name == kind).map_or(0.5, |k| k.size)
    }

    /// The nearest victim a ray meets within `range`, as (entity id, distance); three spheres stand for the body.
    fn ray_victim(&self, origin: [f64; 3], dir: [f64; 3], range: f64, exclude: u32, foes: &str) -> Option<(u32, f64)> {
        self.entities
            .iter()
            .filter(|e| e.id != exclude && self.is_victim(e) && (foes.is_empty() || matches_selector(&self.def, foes, e)))
            .filter_map(|e| {
                let (height, radius) = (self.hit_height(e), self.kind_size(&e.kind));
                [0.2, 0.5, 0.8]
                    .iter()
                    .filter_map(|share| ray_sphere(origin, dir, [e.position[0], e.position[1] + height * share, e.position[2]], radius))
                    .reduce(f64::min)
                    .filter(|t| *t <= range)
                    .map(|t| (e.id, t))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// Where a weapon's shot starts and which way it goes, for one owner.
    fn aim_for(&self, weapon: &WeaponDef, owner: &Entity) -> ([f64; 3], [f64; 3]) {
        let feet_to_muzzle = [owner.position[0], owner.position[1] + weapon.muzzle, owner.position[2]];
        let size = self.kind_size(&owner.kind);
        let from_dir = |dir: [f64; 3]| -> [f64; 3] {
            let flat = (dir[0] * dir[0] + dir[2] * dir[2]).sqrt().max(1e-9);
            [feet_to_muzzle[0] + dir[0] / flat * size, feet_to_muzzle[1], feet_to_muzzle[2] + dir[2] / flat * size]
        };
        let axis = |name: &str| self.camera_axes.get(name).copied();
        match weapon.aim.as_str() {
            "camera" => {
                let (Some(yaw), Some(pitch)) = (axis("camera.yaw"), axis("camera.pitch")) else {
                    let dir = forward(owner.heading, 0.0);
                    return (from_dir(dir), dir);
                };
                let look = forward(yaw, pitch);
                let muzzle = from_dir(look);
                let Some(eye) = axis("camera.x").zip(axis("camera.y")).zip(axis("camera.z")).map(|((x, y), z)| [x, y, z]) else {
                    return (muzzle, look);
                };
                // the crosshair ray: the first wall or victim it meets, else a far point; the shot goes from the muzzle to there
                let wall = self.solid_hit(eye, at(eye, look, FAR));
                let victim = self.ray_victim(eye, look, FAR, owner.id, &weapon.foes).map(|(_, t)| t);
                let reach = [wall, victim].into_iter().flatten().reduce(f64::min).unwrap_or(FAR);
                let point = at(eye, look, reach);
                (muzzle, unit([point[0] - muzzle[0], point[1] - muzzle[1], point[2] - muzzle[2]]))
            }
            "heading" => {
                let dir = forward(owner.heading, 0.0);
                (from_dir(dir), dir)
            }
            selector => {
                let target = self
                    .entities
                    .iter()
                    .filter(|e| e.id != owner.id && matches_selector(&self.def, selector, e))
                    .min_by(|a, b| {
                        let d = |e: &Entity| (e.position[0] - owner.position[0]).powi(2) + (e.position[2] - owner.position[2]).powi(2);
                        d(a).total_cmp(&d(b))
                    });
                let dir = target.map_or_else(
                    || forward(owner.heading, 0.0),
                    |t| {
                        let centre = [t.position[0], t.position[1] + self.hit_height(t) / 2.0, t.position[2]];
                        unit([centre[0] - feet_to_muzzle[0], centre[1] - feet_to_muzzle[1], centre[2] - feet_to_muzzle[2]])
                    },
                );
                (from_dir(dir), dir)
            }
        }
    }

    /// Turns the direction by a random angle up to `spread` degrees either way in yaw and pitch.
    fn scatter(&mut self, dir: [f64; 3], spread: f64) -> [f64; 3] {
        if spread <= 0.0 {
            return dir;
        }
        let (yaw, pitch) = (dir[0].atan2(dir[2]), dir[1].clamp(-1.0, 1.0).asin());
        let reach = spread.to_radians();
        let (dy, dp) = ((self.random() - 0.5) * 2.0 * reach, (self.random() - 0.5) * 2.0 * reach);
        forward(yaw + dy, pitch + dp)
    }

    /// Starts a reload if the weapon has a magazine that is not full.
    pub(super) fn start_reload(&mut self, weapon: usize, owner: u32) {
        let def = &self.def.weapons[weapon];
        let key = format!("{owner}:{}", def.name);
        let (full, seconds) = (def.ammo, def.reload);
        if let Some(state) = self.combat.weapons.get_mut(&key)
            && full > 0
            && state.ammo < full
            && state.reloading == 0
        {
            state.reloading = ticks(seconds).max(1);
        }
    }

    /// Fires a weapon for one owner if it is ready; raises `fired`, or `empty` on a dry trigger.
    pub(super) fn fire_weapon(&mut self, weapon: usize, owner: u32, queue: &mut Vec<Event>) -> bool {
        let def = self.def.weapons[weapon].clone();
        let key = format!("{owner}:{}", def.name);
        let Some(owner_entity) = self.entities.iter().find(|e| e.id == owner).cloned() else { return false };
        let state = self.combat.weapons.entry(key.clone()).or_insert(WeaponState { ammo: def.ammo, cooldown: 0, reloading: 0 });
        if state.reloading > 0 || state.cooldown > 0 {
            return false;
        }
        if def.ammo > 0 && state.ammo == 0 {
            queue.push(Event { name: "empty".into(), a: Some(owner), b: None, depth: 1 });
            return false;
        }
        if def.ammo > 0 {
            state.ammo -= 1;
        }
        state.cooldown = ticks(def.cooldown);
        let (origin, aimed) = self.aim_for(&def, &owner_entity);
        let dir = self.scatter(aimed, def.spread);
        queue.push(Event { name: "fired".into(), a: Some(owner), b: None, depth: 1 });
        if def.mode == "melee" {
            self.swing_melee(&def, &owner_entity, dir, queue);
            return true;
        }
        if def.mode == "hitscan" {
            let wall = self.solid_hit(origin, at(origin, dir, def.range));
            match self.ray_victim(origin, dir, def.range, owner, &def.foes) {
                Some((victim, t)) if wall.is_none_or(|w| t <= w) => {
                    queue.push(Event { name: "hit".into(), a: Some(owner), b: Some(victim), depth: 1 });
                    self.apply_damage(victim, def.damage, Some(owner), queue);
                }
                _ => {
                    if wall.is_some() {
                        queue.push(Event { name: "impact".into(), a: Some(owner), b: None, depth: 1 });
                    }
                }
            }
            return true;
        }
        let Some(shot) = def.shot.as_deref().and_then(|name| self.def.projectiles.iter().find(|p| p.kind == name)).cloned() else { return false };
        let Some(kind) = self.def.kinds.iter().find(|k| k.name == shot.kind) else { return false };
        let mut props = kind.props.clone();
        props.insert("owner".into(), f64::from(owner));
        props.insert("weapon".into(), f64::from(u32::try_from(weapon).unwrap_or(0)));
        props.insert("life".into(), f64::from(ticks(shot.life)));
        props.insert("pierced".into(), 0.0);
        props.insert("last_hit".into(), -1.0);
        self.spawn(&shot.kind, origin, props, dir[0].atan2(dir[2]));
        let id = self.next_id - 1;
        if let Some(entity) = self.entities.iter_mut().find(|e| e.id == id) {
            entity.velocity = [dir[0] * shot.speed, dir[1] * shot.speed, dir[2] * shot.speed];
        }
        true
    }

    /// A melee swing: every victim within the weapon's reach and inside its arc, in front of the owner, is hurt at once.
    fn swing_melee(&mut self, weapon: &WeaponDef, owner: &Entity, dir: [f64; 3], queue: &mut Vec<Event>) {
        let flat = (dir[0] * dir[0] + dir[2] * dir[2]).sqrt().max(1e-9);
        let (fx, fz) = (dir[0] / flat, dir[2] / flat);
        let half_arc = (weapon.arc / 2.0).to_radians();
        let hit: Vec<u32> = self
            .entities
            .iter()
            .filter(|e| e.id != owner.id && self.is_victim(e) && (weapon.foes.is_empty() || matches_selector(&self.def, &weapon.foes, e)))
            .filter(|e| {
                let (dx, dz) = (e.position[0] - owner.position[0], e.position[2] - owner.position[2]);
                let distance = dx.hypot(dz);
                let reach = weapon.range + self.kind_size(&e.kind);
                distance <= reach && (distance < 1e-6 || ((dx * fx + dz * fz) / distance).clamp(-1.0, 1.0).acos() <= half_arc)
            })
            .map(|e| e.id)
            .collect();
        for victim in hit {
            queue.push(Event { name: "hit".into(), a: Some(owner.id), b: Some(victim), depth: 1 });
            self.apply_damage(victim, weapon.damage, Some(owner.id), queue);
        }
    }

    /// Counts the weapons' timers down, reads the fire and reload inputs, and keeps `ammo_NAME` on the owners up to date.
    pub(super) fn weapon_step(&mut self, input: &Inputs, queue: &mut Vec<Event>) {
        self.camera_axes = input.axes.iter().filter(|(name, _)| name.starts_with("camera.")).map(|(n, v)| (n.clone(), *v)).collect();
        for weapon in 0..self.def.weapons.len() {
            let def = self.def.weapons[weapon].clone();
            let owners: Vec<u32> = self.entities.iter().filter(|e| matches_selector(&self.def, &def.owner, e)).map(|e| e.id).collect();
            for owner in owners {
                let key = format!("{owner}:{}", def.name);
                let state = self.combat.weapons.entry(key).or_insert(WeaponState { ammo: def.ammo, cooldown: 0, reloading: 0 });
                state.cooldown = state.cooldown.saturating_sub(1);
                if state.reloading > 0 {
                    state.reloading -= 1;
                    if state.reloading == 0 {
                        state.ammo = def.ammo;
                        queue.push(Event { name: "reloaded".into(), a: Some(owner), b: None, depth: 1 });
                    }
                }
                let pressed = |name: &str| name != "none" && input.pressed.iter().any(|p| p == name);
                if def.reload_input.as_deref().is_some_and(pressed) {
                    self.start_reload(weapon, owner);
                }
                let held = def.auto && input.axes.get(&def.input).is_some_and(|v| *v > 0.0);
                if pressed(&def.input) || held {
                    self.fire_weapon(weapon, owner, queue);
                }
                let ammo = self.combat.weapons.get(&format!("{owner}:{}", def.name)).map(|s| s.ammo);
                if let (Some(ammo), true) = (ammo, def.ammo > 0)
                    && let Some(entity) = self.entities.iter_mut().find(|e| e.id == owner)
                {
                    entity.props.insert(format!("ammo_{}", def.name), f64::from(ammo));
                }
            }
        }
    }

    /// Moves every shot one step and resolves what it meets.
    pub(super) fn advance_projectiles(&mut self, queue: &mut Vec<Event>) {
        if self.def.projectiles.is_empty() {
            return;
        }
        let shots: Vec<u32> = self.entities.iter().filter(|e| self.def.projectiles.iter().any(|p| p.kind == e.kind)).map(|e| e.id).collect();
        for id in shots {
            self.advance_shot(id, queue);
        }
    }

    fn advance_shot(&mut self, id: u32, queue: &mut Vec<Event>) {
        let Some(entity) = self.entities.iter().find(|e| e.id == id).cloned() else { return };
        let Some(shot) = self.def.projectiles.iter().find(|p| p.kind == entity.kind).cloned() else { return };
        let prop = |name: &str| entity.props.get(name).copied().unwrap_or(0.0);
        let owner = id_of(prop("owner"));
        let radius = self.kind_size(&entity.kind);
        let mut velocity = entity.velocity;
        velocity[1] -= shot.gravity * DT;
        if shot.homing > 0.0 {
            velocity = self.steer(&entity, velocity, &shot, owner);
        }
        let next = [entity.position[0] + velocity[0] * DT, entity.position[1] + velocity[1] * DT, entity.position[2] + velocity[2] * DT];
        let wall = self.solid_hit(entity.position, next);
        let last_hit = (prop("last_hit") >= 0.0).then(|| id_of(prop("last_hit")));
        // the weapon that fired it says who it may hurt (a party's shots pass through each other)
        let foes = self.def.weapons.get(usize::try_from(id_of(prop("weapon"))).unwrap_or(usize::MAX)).map(|w| w.foes.clone()).unwrap_or_default();
        // the first victim the step's path comes close enough to
        let length = ((next[0] - entity.position[0]).powi(2) + (next[1] - entity.position[1]).powi(2) + (next[2] - entity.position[2]).powi(2)).sqrt();
        let victim = [0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0].iter().find_map(|share| {
            let point = [
                entity.position[0] + (next[0] - entity.position[0]) * share,
                entity.position[1] + (next[1] - entity.position[1]) * share,
                entity.position[2] + (next[2] - entity.position[2]) * share,
            ];
            self.entities
                .iter()
                .filter(|v| v.id != owner && Some(v.id) != last_hit && self.is_victim(v) && (foes.is_empty() || matches_selector(&self.def, &foes, v)))
                .find(|v| self.distance_to_body(v, point) <= self.kind_size(&v.kind) + radius)
                .map(|v| (v.id, point, share * length))
        });
        if let Some((victim_id, point, along)) = victim
            && wall.is_none_or(|w| along <= w)
        {
            queue.push(Event { name: "hit".into(), a: Some(id), b: Some(victim_id), depth: 1 });
            self.apply_damage(victim_id, shot.damage, Some(owner), queue);
            let pierced = prop("pierced") + 1.0;
            if pierced > f64::from(shot.pierce) {
                self.impact(id, point, Some(victim_id), queue);
                return;
            }
            self.store_shot(id, next, velocity, prop("life") - 1.0, Some(pierced), Some(f64::from(victim_id)));
            return;
        }
        if let Some(distance) = wall {
            let dir = unit([next[0] - entity.position[0], next[1] - entity.position[1], next[2] - entity.position[2]]);
            self.impact(id, at(entity.position, dir, distance), None, queue);
            return;
        }
        let life = prop("life") - 1.0;
        if life <= 0.0 {
            self.impact(id, next, None, queue);
            return;
        }
        self.store_shot(id, next, velocity, life, None, None);
    }

    fn store_shot(&mut self, id: u32, position: [f64; 3], velocity: [f64; 3], life: f64, pierced: Option<f64>, last_hit: Option<f64>) {
        if let Some(entity) = self.entities.iter_mut().find(|e| e.id == id) {
            entity.position = position;
            entity.velocity = velocity;
            entity.props.insert("life".into(), life);
            if let Some(pierced) = pierced {
                entity.props.insert("pierced".into(), pierced);
            }
            if let Some(victim) = last_hit {
                entity.props.insert("last_hit".into(), victim);
            }
        }
    }

    /// Turns a shot's velocity toward the nearest victim at its homing rate, keeping its speed.
    fn steer(&self, entity: &Entity, velocity: [f64; 3], shot: &crate::ProjectileDef, owner: u32) -> [f64; 3] {
        let target = self
            .entities
            .iter()
            .filter(|v| v.id != owner && self.is_victim(v))
            .map(|v| (v, self.distance_to_body(v, entity.position)))
            .filter(|(_, d)| *d < 40.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(v, _)| [v.position[0], v.position[1] + self.hit_height(v) / 2.0, v.position[2]]);
        let Some(goal) = target else { return velocity };
        let (now, want) = (unit(velocity), unit([goal[0] - entity.position[0], goal[1] - entity.position[1], goal[2] - entity.position[2]]));
        let dot = (now[0] * want[0] + now[1] * want[1] + now[2] * want[2]).clamp(-1.0, 1.0);
        let (angle, most) = (dot.acos(), shot.homing.to_radians() * DT);
        let dir = if angle <= most {
            want
        } else {
            let side = unit([want[0] - now[0] * dot, want[1] - now[1] * dot, want[2] - now[2] * dot]);
            unit([now[0] * most.cos() + side[0] * most.sin(), now[1] * most.cos() + side[1] * most.sin(), now[2] * most.cos() + side[2] * most.sin()])
        };
        [dir[0] * shot.speed, dir[1] * shot.speed, dir[2] * shot.speed]
    }

    /// A shot ends at a point: the blast hurts what is near (less with distance, not the victim it just hit, not the shooter), the event
    /// `impact` is raised, and the shot is removed after the rules have heard it.
    fn impact(&mut self, id: u32, point: [f64; 3], direct: Option<u32>, queue: &mut Vec<Event>) {
        let Some(entity) = self.entities.iter().find(|e| e.id == id) else { return };
        let owner = id_of(entity.props.get("owner").copied().unwrap_or(0.0));
        if let Some(shot) = self.def.projectiles.iter().find(|p| p.kind == entity.kind).cloned()
            && shot.splash > 0.0
        {
            let near: Vec<(u32, f64)> = self
                .entities
                .iter()
                .filter(|v| v.id != owner && Some(v.id) != direct && self.is_victim(v))
                .map(|v| (v.id, self.distance_to_body(v, point)))
                .filter(|(_, d)| *d < shot.splash)
                .collect();
            for (victim, distance) in near {
                self.apply_damage(victim, shot.damage * (1.0 - distance / shot.splash), Some(owner), queue);
            }
        }
        queue.push(Event { name: "impact".into(), a: Some(id), b: direct, depth: 1 });
        queue.push(Event { name: format!("reap:{id}"), a: None, b: None, depth: 1 });
    }
}
