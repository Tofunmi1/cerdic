//! Terrain contact, rolling resistance, gravity, and pocket events.
use super::{Entity, Event, World};
use crate::{GroundField, KindDef, SurfaceDef};
use std::{collections::BTreeSet, sync::Arc};

const DT: f64 = 1.0 / 60.0;
const GRAVITY: f64 = 9.8;

impl World {
    /// Attach host terrain; it is excluded from snapshots and hashes.
    pub fn set_ground(&mut self, ground: Option<Arc<GroundField>>) {
        self.ground = ground;
    }

    /// Current host-supplied terrain field.
    #[must_use]
    pub fn ground(&self) -> Option<&Arc<GroundField>> {
        self.ground.as_ref()
    }

    pub(super) fn held_balls(&self) -> BTreeSet<u32> {
        self.hands_state
            .sides
            .iter()
            .filter_map(|side| side.held.as_ref().map(|held| held.id))
            .collect()
    }

    pub(super) fn integrate_rolling(&mut self, queue: &mut Vec<Event>) {
        let held = self.held_balls();
        self.out_balls
            .retain(|id| self.entities.iter().any(|entity| entity.id == *id));
        for entity in &mut self.entities {
            let Some(kind) = self
                .def
                .kinds
                .iter()
                .find(|kind| kind.name == entity.kind && kind.roll)
            else {
                continue;
            };
            if held.contains(&entity.id) {
                continue;
            }
            let ground = self.ground.as_deref();
            let height = height_at(ground, self.def.ground, entity.position);
            if height.is_some_and(|height| entity.position[1] <= height + kind.size + 1e-9)
                && entity.velocity[1].abs() <= f64::EPSILON
            {
                roll(
                    entity,
                    ground,
                    surface_at(ground, &self.def.surfaces, entity.position).0,
                );
                if let Some(height) = height_at(ground, self.def.ground, entity.position) {
                    entity.position[1] = height + kind.size;
                } else {
                    entity.velocity[1] -= GRAVITY * DT;
                    entity.position[1] += entity.velocity[1] * DT;
                }
            } else {
                fall(entity);
                if let Some(height) = height_at(ground, self.def.ground, entity.position)
                    && entity.position[1] <= height + kind.size
                    && entity.velocity[1] < 0.0
                {
                    land(
                        entity,
                        kind,
                        height,
                        surface_at(ground, &self.def.surfaces, entity.position).1,
                    );
                }
            }
            if height_at(ground, self.def.ground, entity.position).is_none()
                && self.out_balls.insert(entity.id)
            {
                queue.push(Event {
                    name: "out".into(),
                    a: Some(entity.id),
                    b: None,
                    depth: 0,
                });
            }
        }
    }

    pub(super) fn collect_pockets(&self, queue: &mut Vec<Event>) {
        let held = self.held_balls();
        for pocket in &self.entities {
            let Some(pocket_kind) = self
                .def
                .kinds
                .iter()
                .find(|kind| kind.name == pocket.kind && kind.pocket)
            else {
                continue;
            };
            for ball in &self.entities {
                let Some(kind) = self
                    .def
                    .kinds
                    .iter()
                    .find(|kind| kind.name == ball.kind && kind.roll)
                else {
                    continue;
                };
                if ball.id == pocket.id || held.contains(&ball.id) {
                    continue;
                }
                let on_ground = height_at(self.ground.as_deref(), self.def.ground, ball.position)
                    .is_some_and(|height| (ball.position[1] - height - kind.size).abs() < 1e-9)
                    && ball.velocity[1].abs() <= f64::EPSILON;
                let speed = ball.velocity.iter().map(|v| v * v).sum::<f64>().sqrt();
                if on_ground
                    && speed < 1.6
                    && (ball.position[0] - pocket.position[0])
                        .hypot(ball.position[2] - pocket.position[2])
                        <= pocket_kind.size
                {
                    queue.push(Event {
                        name: "pocket".into(),
                        a: Some(pocket.id),
                        b: Some(ball.id),
                        depth: 0,
                    });
                }
            }
        }
    }
}

fn height_at(ground: Option<&GroundField>, flat: f64, position: [f64; 3]) -> Option<f64> {
    ground.map_or(Some(flat), |ground| ground.height(position[0], position[2]))
}

fn surface_at(
    ground: Option<&GroundField>,
    surfaces: &[SurfaceDef],
    position: [f64; 3],
) -> (f64, f64) {
    let id = ground.map_or(0, |ground| ground.surface(position[0], position[2]));
    usize::from(id)
        .checked_sub(1)
        .and_then(|i| surfaces.get(i))
        .map_or((1.0, 1.0), |surface| (surface.decel, surface.bounce))
}

fn roll(entity: &mut Entity, ground: Option<&GroundField>, decel: f64) {
    let normal = ground.map_or([0.0, 1.0, 0.0], |ground| {
        ground.normal(entity.position[0], entity.position[2])
    });
    let pull = [
        normal[0] * (5.0 / 7.0) * GRAVITY,
        normal[2] * (5.0 / 7.0) * GRAVITY,
    ];
    let old = [entity.velocity[0], entity.velocity[2]];
    let speed = old[0].hypot(old[1]);
    entity.velocity[1] = 0.0;
    if speed <= decel * DT && pull[0].hypot(pull[1]) <= decel {
        entity.velocity = [0.0; 3];
        return;
    }
    let accelerated = [old[0] + pull[0] * DT, old[1] + pull[1] * DT];
    let magnitude = accelerated[0].hypot(accelerated[1]);
    let scale = if magnitude > 0.0 {
        (1.0 - decel * DT / magnitude).max(0.0)
    } else {
        0.0
    };
    entity.velocity[0] = accelerated[0] * scale;
    entity.velocity[2] = accelerated[1] * scale;
    entity.position[0] += (old[0] + entity.velocity[0]) * 0.5 * DT;
    entity.position[2] += (old[1] + entity.velocity[2]) * 0.5 * DT;
}

fn fall(entity: &mut Entity) {
    entity.velocity[1] -= GRAVITY * DT;
    for axis in 0..3 {
        entity.position[axis] += entity.velocity[axis] * DT;
    }
}

fn land(entity: &mut Entity, kind: &KindDef, height: f64, bounce: f64) {
    entity.position[1] = height + kind.size;
    entity.velocity[1] = -entity.velocity[1] * kind.bounce * bounce;
    if entity.velocity[1] < 0.6 {
        entity.velocity[1] = 0.0;
    }
}
