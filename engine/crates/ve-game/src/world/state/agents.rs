//! Fixed-step autonomous behaviour; navigation is host data, state is rollback data.
use super::{Event, World};
use crate::AgentDef;
use std::sync::Arc;
mod hash;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) enum Mode {
    #[default]
    Patrol,
    Chase,
    Stunned,
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub(super) struct AgentState {
    pub mode: Mode,
    pub stun: u64,
    pub patrol: usize,
    pub caught: bool,
    pub unseen: u64,
    pub path: Vec<[f64; 3]>,
    pub cursor: usize,
    pub goal: Option<[f64; 3]>,
    pub planned: u64,
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] - b[0]).hypot(a[2] - b[2])
}

// NavMesh uses f32 world coordinates. Saturation remains deterministic for finite f64 inputs.
#[expect(
    clippy::cast_possible_truncation,
    reason = "navigation API uses f32 coordinates"
)]
fn point(p: [f64; 3]) -> [f32; 3] {
    p.map(|v| v.clamp(-f64::from(f32::MAX), f64::from(f32::MAX)) as f32)
}

fn raise(queue: &mut Vec<Event>, name: &str, a: u32, b: Option<u32>, depth: usize) {
    queue.push(Event {
        name: name.into(),
        a: Some(a),
        b,
        depth,
    });
}

impl World {
    /// Set the host's walkable mesh. It is excluded from snapshots and hashes.
    pub fn set_nav(&mut self, nav: Option<Arc<ve_nav::NavMesh>>) {
        self.nav = nav;
    }

    /// Current host-provided walkable mesh.
    #[must_use]
    pub fn nav(&self) -> Option<&Arc<ve_nav::NavMesh>> {
        self.nav.as_ref()
    }

    pub(super) fn advance_agents(&mut self, queue: &mut Vec<Event>) {
        let hero = self
            .entities
            .iter()
            .find(|e| self.matches("tag:hero", e))
            .map(|e| (e.id, e.position, self.kind(&e.kind).map_or(0.5, |k| k.size)));
        for index in 0..self.entities.len() {
            let entity = &self.entities[index];
            let Some(def) = self.def.agents.iter().find(|a| a.kind == entity.kind) else {
                continue;
            };
            let id = entity.id;
            let position = entity.position;
            let size = self.kind(&entity.kind).map_or(0.5, |k| k.size);
            let state = self.agents.entry(id).or_default();
            let visible = hero.is_some_and(|(_, p, _)| {
                distance(position, p) <= def.sight
                    && self
                        .nav
                        .as_ref()
                        .is_none_or(|nav| nav.line_of_sight(point(position), point(p)))
            });
            if state.mode == Mode::Stunned {
                state.stun = state.stun.saturating_sub(1);
                if state.stun > 0 {
                    self.entities[index].velocity = [0.0; 3];
                    continue;
                }
                state.mode = if visible { Mode::Chase } else { Mode::Patrol };
                state.unseen = 0;
                state.goal = None;
                if state.mode == Mode::Patrol {
                    nearest_patrol(state, def, position);
                }
                raise(queue, "recover", id, None, 0);
            } else if state.mode == Mode::Patrol && visible {
                state.mode = Mode::Chase;
                state.unseen = 0;
                state.goal = None;
                raise(queue, "sight", id, hero.map(|h| h.0), 0);
            }
            if state.mode == Mode::Chase {
                let in_view = hero.is_some_and(|(_, p, _)| {
                    self.nav
                        .as_ref()
                        .is_none_or(|nav| nav.line_of_sight(point(position), point(p)))
                });
                state.unseen = if in_view { 0 } else { state.unseen + 1 };
                if hero.is_none_or(|(_, p, _)| distance(position, p) > def.lose)
                    || state.unseen >= 180
                {
                    state.mode = Mode::Patrol;
                    state.goal = None;
                    nearest_patrol(state, def, position);
                    raise(queue, "lost", id, hero.map(|h| h.0), 0);
                }
            }
            if let Some((hero_id, p, radius)) = hero {
                let gap = distance(position, p);
                if gap > size + radius + 0.5 {
                    state.caught = false;
                }
                if state.mode == Mode::Chase && gap < size + radius && !state.caught {
                    state.caught = true;
                    raise(queue, "catch", id, Some(hero_id), 0);
                }
            }
            let (goal, speed) = movement_goal(state, def, position, hero.map(|h| h.1));
            let velocity =
                path_velocity(state, self.nav.as_deref(), position, goal, speed, self.tick);
            self.entities[index].velocity = if self.agent_hits_solid(id, position, velocity, size) {
                [0.0; 3]
            } else {
                velocity
            };
        }
    }

    fn agent_hits_solid(&self, id: u32, position: [f64; 3], velocity: [f64; 3], size: f64) -> bool {
        let next = std::array::from_fn(|axis| position[axis] + velocity[axis] / 60.0);
        self.entities.iter().any(|solid| {
            solid.id != id
                && self
                    .kind(&solid.kind)
                    .and_then(|k| k.solid)
                    .is_some_and(|extents| {
                        super::follow::segment_intersects_box(
                            position,
                            next,
                            solid.position,
                            extents.map(|v| v / 2.0 + size - 1e-7),
                        )
                    })
        })
    }

    pub(super) fn stun_action(&mut self, body: &str, event: &Event, queue: &mut Vec<Event>) {
        let Some((selector, seconds)) = body.rsplit_once(':') else {
            return;
        };
        let Ok(seconds) = seconds.parse::<f64>() else {
            return;
        };
        if !seconds.is_finite() || seconds <= 0.0 {
            return;
        }
        for id in self.select_ids(selector, event) {
            let Some(state) = self.agents.get_mut(&id) else {
                continue;
            };
            if state.mode != Mode::Stunned {
                raise(queue, "stun", id, None, event.depth + 1);
            }
            state.mode = Mode::Stunned;
            state.stun = (seconds * 60.0)
                .round()
                .max(1.0)
                .to_string()
                .parse()
                .unwrap_or(u64::MAX);
            state.caught = false;
            state.goal = None;
            if let Some(entity) = self.entities.iter_mut().find(|e| e.id == id) {
                entity.velocity = [0.0; 3];
            }
        }
    }

    pub(super) fn freeze_stunned(&mut self) {
        for entity in &mut self.entities {
            if self
                .agents
                .get(&entity.id)
                .is_some_and(|s| s.mode == Mode::Stunned)
            {
                entity.velocity = [0.0; 3];
            }
        }
    }
}

fn nearest_patrol(state: &mut AgentState, def: &AgentDef, position: [f64; 3]) {
    state.patrol = def
        .patrol
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            distance(position, [a[0], 0.0, a[1]]).total_cmp(&distance(position, [b[0], 0.0, b[1]]))
        })
        .map_or(0, |(index, _)| index);
}

fn movement_goal(
    state: &mut AgentState,
    def: &AgentDef,
    position: [f64; 3],
    hero: Option<[f64; 3]>,
) -> ([f64; 3], f64) {
    if state.mode == Mode::Chase {
        return (hero.unwrap_or(position), def.chase_speed);
    }
    let p = def.patrol[state.patrol];
    if distance(position, [p[0], position[1], p[1]]) <= 0.5 {
        state.patrol = (state.patrol + 1) % def.patrol.len();
        state.goal = None;
    }
    let p = def.patrol[state.patrol];
    ([p[0], position[1], p[1]], def.speed)
}

fn path_velocity(
    state: &mut AgentState,
    nav: Option<&ve_nav::NavMesh>,
    position: [f64; 3],
    goal: [f64; 3],
    speed: f64,
    tick: u64,
) -> [f64; 3] {
    if state.goal.is_none_or(|old| {
        (old[0] - goal[0])
            .hypot(old[1] - goal[1])
            .hypot(old[2] - goal[2])
            > 0.5
    }) || tick.saturating_sub(state.planned) >= 15
    {
        state.goal = Some(goal);
        state.planned = tick;
        state.cursor = 0;
        state.path = nav.map_or_else(
            || vec![goal],
            |nav| {
                nav.find_path(point(position), point(goal))
                    .map(|p| {
                        p.points()
                            .iter()
                            .filter_map(|p| {
                                // Funnel corners can lie exactly on a blocked cell boundary.
                                // Stand inside a walkable cell before steering toward the next corner.
                                if nav.is_walkable(*p) {
                                    Some(*p)
                                } else {
                                    nav.nearest_walkable(*p)
                                }
                            })
                            .map(|p| p.map(f64::from))
                            .collect()
                    })
                    .unwrap_or_default()
            },
        );
        // The first nav corner is the snapped start cell, not a destination to walk back to.
        if state.path.len() > 1
            && nav.is_some_and(|n| n.line_of_sight(point(position), point(state.path[1])))
        {
            state.cursor = 1;
        }
    }
    while state
        .path
        .get(state.cursor)
        .is_some_and(|p| distance(position, *p) < 1e-6)
    {
        state.cursor += 1;
    }
    let Some(target) = state.path.get(state.cursor) else {
        return [0.0; 3];
    };
    let gap = distance(position, *target);
    let scale = speed.min(gap * 60.0) / gap;
    let mut velocity = [
        (target[0] - position[0]) * scale,
        0.0,
        (target[2] - position[2]) * scale,
    ];
    if let Some(nav) = nav {
        let next = [
            position[0] + velocity[0] / 60.0,
            position[1],
            position[2] + velocity[2] / 60.0,
        ];
        if !nav.is_walkable(point(next)) || !nav.line_of_sight(point(position), point(next)) {
            return [0.0; 3];
        }
        if let Some(y) = nav.height_at(point(next)) {
            velocity[1] = (f64::from(y) - position[1]) * 60.0;
        }
    }
    velocity
}
