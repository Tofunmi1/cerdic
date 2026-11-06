//! Hand poses, deterministic grabbing, carrying, release, and use events.
use super::{Event, World};
use crate::{HandInput, HandSide};

pub(super) fn valid_positions(input: &crate::Inputs) -> bool {
    input
        .hands
        .iter()
        .all(|hand| hand.position.iter().all(|value| value.is_finite()))
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) struct HandsState {
    pub(super) sides: [SideState; 2],
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) struct SideState {
    pub(super) grip: bool,
    pub(super) trigger: bool,
    pub(super) held: Option<HeldItem>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) struct HeldItem {
    pub(super) id: u32,
    pub(super) offset: [f64; 3],
}

impl World {
    pub(super) fn update_hands(&mut self, samples: &[HandInput], queue: &mut Vec<Event>) {
        for side in [HandSide::Left, HandSide::Right] {
            self.update_hand(
                side,
                samples.iter().find(|sample| sample.side == side),
                queue,
            );
        }
    }

    fn update_hand(&mut self, side: HandSide, sample: Option<&HandInput>, queue: &mut Vec<Event>) {
        let index = match side {
            HandSide::Left => 0,
            HandSide::Right => 1,
        };
        let hand_id = self
            .entities
            .iter()
            .find(|entity| {
                self.def.inputs.iter().any(|input| {
                    input.hand_follow == Some(side)
                        && input.target.as_deref().is_some_and(|selector| {
                            super::follow::matches_selector(&self.def, selector, entity)
                        })
                })
            })
            .map(|entity| entity.id);
        let Some(hand_id) = hand_id else {
            if let Some(held) = self.hands_state.sides[index].held.take() {
                self.release_velocity(held.id);
            }
            self.hands_state.sides[index].grip = false;
            self.hands_state.sides[index].trigger = false;
            return;
        };
        let Some(sample) = sample else {
            if let Some(held) = self.hands_state.sides[index].held.take() {
                self.release_velocity(held.id);
                if self.entities.iter().any(|entity| entity.id == held.id) {
                    queue.push(event("release", hand_id, held.id));
                }
            }
            self.hands_state.sides[index].grip = false;
            self.hands_state.sides[index].trigger = false;
            return;
        };
        if self.hands_state.sides[index]
            .held
            .as_ref()
            .is_some_and(|held| !self.entities.iter().any(|entity| entity.id == held.id))
        {
            self.hands_state.sides[index].held = None;
        }
        let previous = self.hands_state.sides[index].clone();
        if previous.grip
            && !sample.grip
            && let Some(held) = self.hands_state.sides[index].held.take()
        {
            self.release_velocity(held.id);
            queue.push(event("release", hand_id, held.id));
        }
        if !previous.grip
            && sample.grip
            && self.hands_state.sides[index].held.is_none()
            && let Some((id, position)) = self.nearest_grabbable(hand_id, sample.position)
        {
            let offset = subtract(position, sample.position);
            self.hands_state.sides[index].held = Some(HeldItem { id, offset });
            queue.push(event("grab", hand_id, id));
        }
        if sample.grip {
            self.carry_held(index, sample.position);
        }
        if !previous.trigger && sample.trigger {
            let target = self.nearest_usable(hand_id, sample.position);
            queue.push(Event {
                name: "use".into(),
                a: Some(hand_id),
                b: target,
                depth: 0,
            });
        }
        self.hands_state.sides[index].grip = sample.grip;
        self.hands_state.sides[index].trigger = sample.trigger;
    }

    fn carry_held(&mut self, index: usize, hand_position: [f64; 3]) {
        let Some(held) = &self.hands_state.sides[index].held else {
            return;
        };
        let (id, offset) = (held.id, held.offset);
        if let Some(item) = self.entities.iter_mut().find(|entity| entity.id == id) {
            item.position = add(hand_position, offset);
        }
        if let Some(position) = self
            .entities
            .iter()
            .find(|entity| entity.id == id)
            .map(|entity| entity.position)
        {
            let history = self.motion_history.entry(id).or_default();
            history.push(position);
            if history.len() > 5 {
                history.remove(0);
            }
        }
    }

    fn nearest_grabbable(&self, hand_id: u32, position: [f64; 3]) -> Option<(u32, [f64; 3])> {
        let held_elsewhere = self
            .hands_state
            .sides
            .iter()
            .filter_map(|state| state.held.as_ref().map(|held| held.id))
            .collect::<Vec<_>>();
        self.entities
            .iter()
            .filter_map(|entity| {
                if entity.id == hand_id || held_elsewhere.contains(&entity.id) {
                    return None;
                }
                let kind = self
                    .def
                    .kinds
                    .iter()
                    .find(|kind| kind.name == entity.kind)?;
                let hand = self.entity_kind(hand_id)?;
                let distance = distance(position, entity.position);
                (kind.grab && distance <= hand.size + kind.size).then_some((
                    entity.id,
                    entity.position,
                    distance,
                ))
            })
            .min_by(|left, right| {
                left.2
                    .total_cmp(&right.2)
                    .then_with(|| left.0.cmp(&right.0))
            })
            .map(|(id, position, _)| (id, position))
    }

    fn nearest_usable(&self, hand_id: u32, position: [f64; 3]) -> Option<u32> {
        let hand_size = self.entity_kind(hand_id).map_or(0.5, |kind| kind.size);
        self.entities
            .iter()
            .filter_map(|entity| {
                if entity.id == hand_id {
                    return None;
                }
                let size = self
                    .def
                    .kinds
                    .iter()
                    .find(|kind| kind.name == entity.kind)?
                    .size;
                let distance = distance(position, entity.position);
                (distance <= hand_size + size + 0.05).then_some((entity.id, distance))
            })
            .min_by(|left, right| {
                left.1
                    .total_cmp(&right.1)
                    .then_with(|| left.0.cmp(&right.0))
            })
            .map(|(id, _)| id)
    }

    fn entity_kind(&self, id: u32) -> Option<&crate::KindDef> {
        let entity = self.entities.iter().find(|entity| entity.id == id)?;
        self.def.kinds.iter().find(|kind| kind.name == entity.kind)
    }
}

impl World {
    fn release_velocity(&mut self, id: u32) {
        let falls = self
            .entities
            .iter()
            .find(|entity| entity.id == id)
            .and_then(|entity| self.def.kinds.iter().find(|kind| kind.name == entity.kind))
            .is_some_and(|kind| kind.fall);
        let Some(history) = self.motion_history.remove(&id) else {
            return;
        };
        if !falls {
            return;
        }
        let Some(entity) = self.entities.iter_mut().find(|entity| entity.id == id) else {
            return;
        };
        if let (Some(first), Some(last)) = (history.first(), history.last()) {
            let intervals = u32::try_from(history.len().saturating_sub(1))
                .unwrap_or(1)
                .max(1);
            let divisor = f64::from(intervals) / 60.0;
            entity.velocity = [
                (last[0] - first[0]) / divisor,
                (last[1] - first[1]) / divisor,
                (last[2] - first[2]) / divisor,
            ];
        }
    }
}

fn event(name: &str, a: u32, b: u32) -> Event {
    Event {
        name: name.into(),
        a: Some(a),
        b: Some(b),
        depth: 0,
    }
}
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}
fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn subtract(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
