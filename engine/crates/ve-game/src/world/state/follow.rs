//! Position updates that bind selected entities to the viewer.
use super::{Entity, World};
use crate::GameDef;

impl World {
    pub(super) fn apply_viewer_follow(&mut self, viewer: Option<[f64; 3]>) {
        let Some(position) = viewer else {
            return;
        };
        let def = &self.def;
        for entity in &mut self.entities {
            let follows = def.inputs.iter().any(|declaration| {
                declaration.follow
                    && declaration
                        .target
                        .as_deref()
                        .is_some_and(|selector| matches_selector(def, selector, entity))
            });
            if follows {
                entity.position = position;
            }
        }
    }

    pub(super) fn apply_hand_follow(&mut self, input: &crate::Inputs) {
        let def = &self.def;
        for entity in &mut self.entities {
            let side = def.inputs.iter().find_map(|declaration| {
                declaration.hand_follow.filter(|_| {
                    declaration
                        .target
                        .as_deref()
                        .is_some_and(|selector| matches_selector(def, selector, entity))
                })
            });
            if let Some(position) = side.and_then(|side| {
                input
                    .hands
                    .iter()
                    .find(|hand| hand.side == side)
                    .map(|hand| hand.position)
            }) {
                entity.position = position;
                entity.orientation = input
                    .hand_rotations
                    .iter()
                    .find(|rotation| Some(rotation.side) == side)
                    .map_or([0.0, 0.0, 0.0, 1.0], |rotation| rotation.rotation);
            } else if side.is_some() {
                entity.orientation = [0.0, 0.0, 0.0, 1.0];
            }
        }
    }

    pub(super) fn resolve_viewer_solids(&mut self, previous_positions: &[(u32, [f64; 3])]) {
        let def = &self.def;
        let solids = self
            .entities
            .iter()
            .filter_map(|entity| {
                let kind = def.kinds.iter().find(|kind| kind.name == entity.kind)?;
                Some((entity.id, entity.position, kind.solid?))
            })
            .collect::<Vec<_>>();
        for entity in &mut self.entities {
            let follows = def.inputs.iter().any(|declaration| {
                declaration.follow
                    && declaration
                        .target
                        .as_deref()
                        .is_some_and(|selector| matches_selector(def, selector, entity))
            });
            if !follows {
                continue;
            }
            let radius = def
                .kinds
                .iter()
                .find(|kind| kind.name == entity.kind)
                .map_or(0.5, |kind| kind.size);
            for (solid_id, center, extents) in &solids {
                if entity.id == *solid_id {
                    continue;
                }
                let half_x = extents[0] / 2.0 + radius;
                let half_y = extents[1] / 2.0 + radius;
                let half_z = extents[2] / 2.0 + radius;
                let dx = entity.position[0] - center[0];
                let dz = entity.position[2] - center[2];
                let previous = previous_positions
                    .iter()
                    .find(|(id, _)| *id == entity.id)
                    .map_or(entity.position, |(_, position)| *position);
                // Resting exactly on a face (where a push leaves the hero) is not inside: a shrunken box keeps the hero free to walk away.
                if !segment_intersects_box(
                    previous,
                    entity.position,
                    *center,
                    [
                        half_x - FACE_MARGIN,
                        half_y - FACE_MARGIN,
                        half_z - FACE_MARGIN,
                    ],
                ) {
                    continue;
                }
                let penetration_x = half_x - dx.abs();
                let penetration_z = half_z - dz.abs();
                let axis = if penetration_x <= penetration_z { 0 } else { 2 };
                let (delta, half) = if axis == 0 {
                    (dx, half_x)
                } else {
                    (dz, half_z)
                };
                let prior_delta = previous[axis] - center[axis];
                // A hero that came from outside the box (or from resting on its face) is held on the side it came from, however far it
                // jumped; one that started inside is pushed out the way it now leans.
                let side = if prior_delta.abs() >= half - 1e-9 {
                    prior_delta
                } else {
                    delta
                };
                entity.position[axis] = center[axis] + if side <= 1e-9 { -half } else { half };
            }
        }
    }
}

const FACE_MARGIN: f64 = 1e-7;

pub(super) fn segment_intersects_box(
    start: [f64; 3],
    end: [f64; 3],
    center: [f64; 3],
    half_extents: [f64; 3],
) -> bool {
    let mut first = 0.0_f64;
    let mut last = 1.0_f64;
    for axis in 0..3 {
        let delta = end[axis] - start[axis];
        let min = center[axis] - half_extents[axis];
        let max = center[axis] + half_extents[axis];
        if delta.abs() <= f64::EPSILON {
            if start[axis] < min || start[axis] > max {
                return false;
            }
            continue;
        }
        let near = (min - start[axis]) / delta;
        let far = (max - start[axis]) / delta;
        first = first.max(near.min(far));
        last = last.min(near.max(far));
        if first > last {
            return false;
        }
    }
    true
}

pub(crate) fn matches_selector(def: &GameDef, selector: &str, entity: &Entity) -> bool {
    if let Some(kind) = selector.strip_prefix("kind:") {
        return entity.kind == kind;
    }
    if let Some(tag) = selector.strip_prefix("tag:") {
        return def
            .kinds
            .iter()
            .find(|kind| kind.name == entity.kind)
            .is_some_and(|kind| kind.tags.iter().any(|value| value == tag));
    }
    true
}
