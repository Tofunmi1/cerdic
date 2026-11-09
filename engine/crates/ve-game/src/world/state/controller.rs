//! Character controllers: dynamics, gravity, jumping and platforms for the kinds declared with a `controller` line.
//!
//! The entity's `y` is its feet. Each fixed step: the horizontal velocity moves toward the input (acceleration, deceleration, air control),
//! a jump fires from the ground, within the coyote window, from the buffer on landing, or as an extra jump in the air, gravity pulls, and the
//! feet land on the highest surface under them (the ground field or flat ground, or the top of a `solid` box no more than a step above).
//! A box taller than a step blocks the sides. The numbers a rule or a script may want are left on the entity as props: `grounded` (1 or 0)
//! and `speed` (horizontal, metres per second). The input names the controller reads are `NAME.x` and `NAME.z` (axes), the jump press, and
//! `camera.yaw` (radians, the direction the camera looks along, supplied by the host) when the movement is camera-relative.
use std::sync::Arc;

use super::{Entity, World};
use crate::{ControllerDef, GroundField, Inputs};

const DT: f64 = 1.0 / 60.0;
const EPSILON: f64 = 1e-6;

/// A solid box in the world: its centre and half extents.
struct Platform {
    id: u32,
    centre: [f64; 3],
    half: [f64; 3],
}

/// What the controlled entities stand on.
struct Surroundings {
    ground: Option<Arc<GroundField>>,
    flat: f64,
    platforms: Vec<Platform>,
}

impl Surroundings {
    /// The height of the highest surface under `(x, z)` that a body with its feet at `feet` can stand on: the ground, or the top of a box
    /// that is no more than `reach` above the feet.
    fn surface(&self, x: f64, z: f64, feet: f64, reach: f64) -> f64 {
        let mut height = self.ground.as_ref().and_then(|field| field.height(x, z)).unwrap_or(self.flat);
        for platform in &self.platforms {
            if (x - platform.centre[0]).abs() <= platform.half[0] && (z - platform.centre[2]).abs() <= platform.half[2] {
                let top = platform.centre[1] + platform.half[1];
                if top <= feet + reach && top > height {
                    height = top;
                }
            }
        }
        height
    }
}

impl World {
    /// Moves every entity of a controlled kind one fixed step from the input.
    pub(super) fn integrate_controllers(&mut self, input: &Inputs, queue: &mut Vec<super::Event>) {
        if self.def.controllers.is_empty() {
            return;
        }
        let controllers = self.def.controllers.clone();
        let platforms = self
            .entities
            .iter()
            .filter_map(|entity| {
                let extents = self.def.kinds.iter().find(|kind| kind.name == entity.kind)?.solid?;
                Some(Platform { id: entity.id, centre: entity.position, half: [extents[0] / 2.0, extents[1] / 2.0, extents[2] / 2.0] })
            })
            .collect();
        let around = Surroundings { ground: self.ground.clone(), flat: self.def.ground, platforms };
        let camera_yaw = input.axes.get("camera.yaw").copied().unwrap_or(0.0);
        for entity in &mut self.entities {
            let Some(controller) = controllers.iter().find(|c| c.kind == entity.kind) else {
                continue;
            };
            let radius = self.def.kinds.iter().find(|kind| kind.name == entity.kind).map_or(0.3, |kind| kind.size);
            if step_one(entity, controller, radius, input, camera_yaw, &around) {
                queue.push(super::Event { name: "fell".into(), a: Some(entity.id), b: None, depth: 0 });
            }
        }
    }
}

fn prop(entity: &Entity, key: &str) -> f64 {
    entity.props.get(key).copied().unwrap_or(0.0)
}

/// The world-space direction (not longer than 1) the input asks for.
fn wanted_direction(c: &ControllerDef, input: &Inputs, camera_yaw: f64) -> (f64, f64) {
    let axis = |name: &str| input.axes.get(&format!("{}.{name}", c.movement)).copied().unwrap_or(0.0).clamp(-1.0, 1.0);
    let (x, z) = (axis("x"), axis("z"));
    // the camera looks along (sin yaw, cos yaw); its right is (-cos yaw, sin yaw)
    let (mut dx, mut dz) = if c.relative == "camera" {
        let (sine, cosine) = camera_yaw.sin_cos();
        (-cosine * x + sine * z, sine * x + cosine * z)
    } else {
        (x, z)
    };
    match c.lock.as_str() {
        "x" => dx = 0.0,
        "z" => dz = 0.0,
        _ => {}
    }
    let length = dx.hypot(dz);
    if length > 1.0 {
        (dx / length, dz / length)
    } else {
        (dx, dz)
    }
}

/// Pushes `(x, z)` out of every box taller than a step that the body would otherwise walk into, and stops the velocity into it.
fn block_sides(entity_id: u32, feet: f64, radius: f64, step: f64, (x, z): (&mut f64, &mut f64), (vx, vz): (&mut f64, &mut f64), around: &Surroundings, before: (f64, f64)) {
    for platform in &around.platforms {
        if platform.id == entity_id || feet >= platform.centre[1] + platform.half[1] - step - EPSILON {
            continue;
        }
        let (reach_x, reach_z) = (platform.half[0] + radius, platform.half[2] + radius);
        let (dx, dz) = (*x - platform.centre[0], *z - platform.centre[2]);
        if dx.abs() >= reach_x || dz.abs() >= reach_z {
            continue;
        }
        let (push_x, push_z) = (reach_x - dx.abs(), reach_z - dz.abs());
        if push_x < push_z {
            let side = if dx.abs() > EPSILON { dx.signum() } else { (before.0 - platform.centre[0]).signum() };
            *x = platform.centre[0] + side * reach_x;
            *vx = 0.0;
        } else {
            let side = if dz.abs() > EPSILON { dz.signum() } else { (before.1 - platform.centre[2]).signum() };
            *z = platform.centre[2] + side * reach_z;
            *vz = 0.0;
        }
    }
}

fn wrap(angle: f64) -> f64 {
    let turn = std::f64::consts::TAU;
    (angle + std::f64::consts::PI).rem_euclid(turn) - std::f64::consts::PI
}

/// One fixed step of one entity; returns whether it fell below its limit and was put back.
fn step_one(entity: &mut Entity, c: &ControllerDef, radius: f64, input: &Inputs, camera_yaw: f64, around: &Surroundings) -> bool {
    let (dx, dz) = wanted_direction(c, input, camera_yaw);
    let [px, feet0, pz] = entity.position;
    let mut vy = entity.velocity[1];
    // standing on something at the start of the step
    let grounded_at_start = feet0 <= around.surface(px, pz, feet0, c.step) + EPSILON && vy <= 0.0;

    // horizontal velocity toward what the input asks for
    let rate = if dx.hypot(dz) > 0.0 { c.accel } else { c.decel } * if grounded_at_start { 1.0 } else { c.air };
    let (target_x, target_z) = (dx * c.speed, dz * c.speed);
    let (error_x, error_z) = (target_x - entity.velocity[0], target_z - entity.velocity[2]);
    let (distance, most) = (error_x.hypot(error_z), rate * DT);
    let (mut vx, mut vz) = if distance <= most {
        (target_x, target_z)
    } else {
        (entity.velocity[0] + error_x / distance * most, entity.velocity[2] + error_z / distance * most)
    };
    // a dash (the `dash` action, a dodge) overrides the input for its few steps: the entity is carried at the dash velocity
    let dashing = prop(entity, "dash_left");
    if dashing > 0.0 {
        (vx, vz) = (prop(entity, "dash_vx"), prop(entity, "dash_vz"));
        entity.props.insert("dash_left".into(), dashing - 1.0);
    }

    // jump bookkeeping: the press edge fills the buffer, a release cuts a rising jump, the ground refills the coyote window and extra jumps
    // The key is down when it was pressed this step or an axis of the same name is above zero (the page sets that axis while the key is
    // held, so a held key is not mistaken for a release between presses). A host that only sends presses holds the key by pressing every step.
    let pressed = input.pressed.contains(&c.jump_input) || input.axes.get(&c.jump_input).is_some_and(|value| *value > 0.0);
    let was_held = prop(entity, "ctl_held") > 0.0;
    let mut buffer = if pressed && !was_held { c.buffer } else { (prop(entity, "ctl_buffer") - DT).max(0.0) };
    if !pressed && was_held && vy > 0.0 {
        vy *= c.cut;
    }
    let (mut coyote, mut jumps_left) = (prop(entity, "ctl_coyote"), prop(entity, "ctl_jumps"));
    if grounded_at_start {
        coyote = c.coyote;
        jumps_left = f64::from(c.extra_jumps);
        vy = 0.0;
    } else {
        coyote = (coyote - DT).max(0.0);
    }
    let mut jumped = false;
    if c.jump > 0.0 && buffer > 0.0 {
        if grounded_at_start || coyote > 0.0 {
            (vy, jumped, buffer, coyote) = (c.jump, true, 0.0, 0.0);
        } else if jumps_left >= 1.0 {
            (vy, jumped, buffer) = (c.jump, true, 0.0);
            jumps_left -= 1.0;
        }
    }

    // move: sideways first (boxes taller than a step block it), then up or down
    let (mut x, mut z) = (px + vx * DT, pz + vz * DT);
    block_sides(entity.id, feet0, radius, c.step, (&mut x, &mut z), (&mut vx, &mut vz), around, (px, pz));
    let airborne = jumped || !grounded_at_start;
    let mut feet = feet0;
    if airborne {
        vy -= c.gravity * DT;
        feet += vy * DT;
    }
    // land on the highest surface under the new place, or step up onto a low one
    let floor = around.surface(x, z, feet0, c.step);
    let mut grounded = false;
    if vy <= 0.0 && feet <= floor + EPSILON {
        feet = floor;
        vy = 0.0;
        grounded = true;
        if airborne && c.jump > 0.0 && buffer > 0.0 {
            // a jump pressed just before touching down goes off the moment it lands
            (vy, grounded, buffer) = (c.jump, false, 0.0);
        }
    }

    // turn toward the motion
    if c.turn > 0.0 && vx.hypot(vz) > 0.1 {
        let most_turn = c.turn.to_radians() * DT;
        entity.heading += wrap(vx.atan2(vz) - entity.heading).clamp(-most_turn, most_turn);
    }

    entity.position = [x, feet, z];
    entity.velocity = [vx, vy, vz];
    let flag = |on: bool| if on { 1.0 } else { 0.0 };
    entity.props.insert("grounded".into(), flag(grounded));
    entity.props.insert("speed".into(), vx.hypot(vz));
    entity.props.insert("ctl_held".into(), flag(pressed));
    entity.props.insert("ctl_buffer".into(), buffer);
    entity.props.insert("ctl_coyote".into(), coyote);
    entity.props.insert("ctl_jumps".into(), jumps_left);
    keep_safe_ground(entity, grounded, c.fall_limit)
}

/// Samples the ground the entity stands on every half second (keeping the sample before it, which lies further from any edge the entity
/// then walked off), and when the entity has fallen below its limit puts it back on the older sample, at rest. Returns whether it fell.
fn keep_safe_ground(entity: &mut Entity, grounded: bool, limit: Option<f64>) -> bool {
    let steady = if grounded { prop(entity, "ctl_steady") + 1.0 } else { 0.0 };
    entity.props.insert("ctl_steady".into(), steady);
    if grounded && steady % 30.0 == 0.0 {
        for (axis, value) in [("x", entity.position[0]), ("y", entity.position[1]), ("z", entity.position[2])] {
            let latest = entity.props.insert(format!("ctl_safe_{axis}"), value);
            entity.props.insert(format!("ctl_safe_old_{axis}"), latest.unwrap_or(value));
        }
    }
    if !limit.is_some_and(|limit| entity.position[1] < limit) {
        return false;
    }
    let (Some(x), Some(y), Some(z)) = (
        entity.props.get("ctl_safe_old_x").copied(),
        entity.props.get("ctl_safe_old_y").copied(),
        entity.props.get("ctl_safe_old_z").copied(),
    ) else {
        return false;
    };
    entity.position = [x, y, z];
    entity.velocity = [0.0; 3];
    entity.props.insert("ctl_steady".into(), 0.0);
    true
}
