//! Swept tool heads, speed histories, and per-ball swing latches.
use super::{Event, World, follow::matches_selector};
use crate::{Inputs, KindDef};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) struct ToolState {
    pub(super) head: [f64; 3],
    pub(super) history: Vec<[f64; 3]>,
}

impl World {
    fn tracked_tools(&self, input: &Inputs) -> Vec<(u32, [f64; 3], KindDef)> {
        self.entities
            .iter()
            .filter_map(|entity| {
                let kind = self
                    .def
                    .kinds
                    .iter()
                    .find(|kind| kind.name == entity.kind && kind.swing)?;
                let side = self.def.inputs.iter().find_map(|declaration| {
                    declaration.hand_follow.filter(|_| {
                        declaration
                            .target
                            .as_deref()
                            .is_some_and(|target| matches_selector(&self.def, target, entity))
                    })
                })?;
                input.hands.iter().find(|hand| hand.side == side)?;
                Some((
                    entity.id,
                    head_position(entity.position, entity.orientation, kind.reach),
                    kind.clone(),
                ))
            })
            .collect()
    }

    pub(super) fn swing_balls(&mut self, input: &Inputs, queue: &mut Vec<Event>) {
        let held = self.held_balls();
        let tools = self.tracked_tools(input);
        self.tools
            .retain(|id, _| tools.iter().any(|(tool, _, _)| tool == id));
        self.swing_latches.retain(|(tool, ball)| {
            self.tools.contains_key(tool) && self.entities.iter().any(|entity| entity.id == *ball)
        });
        for (id, head, kind) in tools {
            let Some(state) = self.tools.get_mut(&id) else {
                self.tools.insert(
                    id,
                    ToolState {
                        head,
                        history: vec![],
                    },
                );
                continue;
            };
            let start = state.head;
            let delta = [head[0] - start[0], head[1] - start[1], head[2] - start[2]];
            state.head = head;
            state.history.push(start);
            if state.history.len() > 3 {
                state.history.remove(0);
            }
            let count = u32::try_from(state.history.len()).unwrap_or(3);
            let travelled = state
                .history
                .iter()
                .zip(state.history.iter().skip(1).chain(std::iter::once(&head)))
                .map(|(a, b)| distance(*a, *b))
                .sum::<f64>();
            let speed = travelled * 60.0 / f64::from(count);
            let horizontal = delta[0].hypot(delta[2]);
            for ball in &mut self.entities {
                let Some(ball_kind) = self
                    .def
                    .kinds
                    .iter()
                    .find(|kind| kind.name == ball.kind && kind.roll)
                else {
                    continue;
                };
                if ball.id == id || held.contains(&ball.id) {
                    continue;
                }
                let pair = (id, ball.id);
                // Compare simultaneous poses from the previous step. The new head paired
                // with the ball before integration can falsely rearm a fast capped swing.
                if distance(start, ball.position) > 0.3 || speed < 0.3 {
                    self.swing_latches.remove(&pair);
                }
                if speed < 0.4
                    || horizontal <= f64::EPSILON
                    || self.swing_latches.contains(&pair)
                    || segment_distance(start, head, ball.position) > kind.size + ball_kind.size
                {
                    continue;
                }
                let launch = (speed * kind.power).min(45.0);
                let loft = kind.loft.to_radians();
                ball.velocity = [
                    delta[0] / horizontal * launch * loft.cos(),
                    launch * loft.sin(),
                    delta[2] / horizontal * launch * loft.cos(),
                ];
                self.swing_latches.insert(pair);
                queue.push(Event {
                    name: "swing".into(),
                    a: Some(id),
                    b: Some(ball.id),
                    depth: 0,
                });
            }
        }
    }
}

fn head_position(position: [f64; 3], rotation: [f64; 4], reach: f64) -> [f64; 3] {
    let [x, y, z, w] = rotation;
    [
        position[0] - reach * 2.0 * (x * z + w * y),
        position[1] - reach * 2.0 * (y * z - w * x),
        position[2] - reach * (1.0 - 2.0 * (x * x + y * y)),
    ]
}

fn length(v: [f64; 3]) -> f64 {
    v.iter().map(|v| v * v).sum::<f64>().sqrt()
}
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    length([a[0] - b[0], a[1] - b[1], a[2] - b[2]])
}

fn segment_distance(start: [f64; 3], end: [f64; 3], point: [f64; 3]) -> f64 {
    let delta = [end[0] - start[0], end[1] - start[1], end[2] - start[2]];
    let squared = delta.iter().map(|v| v * v).sum::<f64>();
    let projection = (0..3)
        .map(|i| (point[i] - start[i]) * delta[i])
        .sum::<f64>();
    let t = if squared > 0.0 {
        (projection / squared).clamp(0.0, 1.0)
    } else {
        0.0
    };
    distance(
        [
            start[0] + t * delta[0],
            start[1] + t * delta[1],
            start[2] + t * delta[2],
        ],
        point,
    )
}
