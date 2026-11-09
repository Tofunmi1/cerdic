//! Entity integration and declared movement inputs.
use super::{World, follow::matches_selector};
use crate::{Inputs, Status};
use std::collections::BTreeSet;

const DT: f64 = 1.0 / 60.0;

impl World {
    pub(super) fn update_headings_and_status(&mut self, starting_positions: &[(u32, [f64; 3])]) {
        for entity in &mut self.entities {
            // a controller that turns at a set rate looks after its own heading
            if self.def.controllers.iter().any(|c| c.kind == entity.kind && c.turn > 0.0) {
                continue;
            }
            if let Some((_, start)) = starting_positions.iter().find(|(id, _)| *id == entity.id) {
                let dx = entity.position[0] - start[0];
                let dz = entity.position[2] - start[2];
                if dx.hypot(dz) >= 1e-9 {
                    entity.heading = dx.atan2(dz);
                }
            }
        }
        for goal in &self.def.goals {
            if goal.expr.eval(&|name| self.lookup(name, None)) != 0.0 {
                self.status = if goal.win { Status::Won } else { Status::Lost };
                break;
            }
        }
    }

    pub(super) fn integrate_falling(&mut self, released: &BTreeSet<u32>) {
        let held = self
            .hands_state
            .sides
            .iter()
            .filter_map(|side| side.held.as_ref().map(|item| item.id))
            .collect::<BTreeSet<_>>();
        for entity in &mut self.entities {
            let Some(kind) = self
                .def
                .kinds
                .iter()
                .find(|kind| kind.name == entity.kind && kind.fall && !kind.roll)
            else {
                continue;
            };
            if held.contains(&entity.id) || released.contains(&entity.id) {
                continue;
            }
            entity.velocity[1] -= 9.8 / 60.0;
            for axis in 0..3 {
                entity.position[axis] += entity.velocity[axis] / 60.0;
            }
            if entity.position[1] - kind.size < self.def.ground {
                entity.position[1] = self.def.ground + kind.size;
                entity.velocity[1] = -entity.velocity[1] * kind.bounce;
                if entity.velocity[1].abs() < 0.5 {
                    entity.velocity[1] = 0.0;
                }
                entity.velocity[0] *= 0.9;
                entity.velocity[2] *= 0.9;
                if entity.velocity[0].hypot(entity.velocity[2]) < 0.02 {
                    entity.velocity[0] = 0.0;
                    entity.velocity[2] = 0.0;
                }
            }
        }
    }

    pub(super) fn integrate(&mut self, input: &Inputs) {
        let def = &self.def;
        for entity in &mut self.entities {
            let kind = def.kinds.iter().find(|kind| kind.name == entity.kind);
            if kind.is_some_and(|kind| kind.fall || kind.roll) {
                continue;
            }
            // a controlled kind is moved by its controller, with dynamics, not by the plain axes
            if def.controllers.iter().any(|c| c.kind == entity.kind) {
                continue;
            }
            // a shot is flown by `advance_projectiles` (gravity, homing, hits), not drifted by its velocity
            if def.projectiles.iter().any(|shot| shot.kind == entity.kind) {
                continue;
            }
            for axis in 0..3 {
                entity.position[axis] += entity.velocity[axis] * DT;
            }
            if let Some(kind) = kind {
                for declaration in &self.def.inputs {
                    if declaration
                        .target
                        .as_deref()
                        .is_some_and(|selector| !matches_selector(def, selector, entity))
                    {
                        continue;
                    }
                    for axis_name in &declaration.axes {
                        let axis = format!("{}.{}", declaration.name, axis_name);
                        if let Some(value) = input.axes.get(&axis) {
                            let coordinate = match axis_name.as_str() {
                                "x" => 0,
                                "y" => 1,
                                "z" => 2,
                                _ => continue,
                            };
                            entity.position[coordinate] += value.clamp(-1.0, 1.0) * kind.speed * DT;
                        }
                    }
                }
            }
        }
    }
}
