//! A deterministic crowd that follows navigation paths.

use glam::Vec2;

use crate::agent::{Agent, AgentParams, AgentStatus, repath, validate};
use crate::error::NavError;
use crate::mesh::NavMesh;
use crate::steering::{self, AVOID_HORIZON, SEPARATION_RANGE};

/// Most agents one crowd can hold.
pub const MAX_AGENTS: usize = 256;

/// Overlap-relaxation passes per step.
const RELAX_PASSES: usize = 12;
/// Steps without movement before an agent is declared blocked.
const STUCK_STEPS: u32 = 900;

/// A group of agents that walk the same [`NavMesh`].
///
/// [`Crowd::step`] is deterministic: agents are processed in index order, every scratch buffer is a
/// `Vec`, and no hash map is ever iterated. Two runs with the same inputs produce the same state.
pub struct Crowd {
    nav: NavMesh,
    agents: Vec<Agent>,
}

impl Crowd {
    /// A crowd that steers on a copy of `nav`.
    #[must_use]
    pub fn new(nav: NavMesh) -> Self {
        Self {
            nav,
            agents: Vec::new(),
        }
    }

    /// The mesh the crowd walks on.
    #[must_use]
    pub fn nav(&self) -> &NavMesh {
        &self.nav
    }

    /// How many agents the crowd holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.agents.len()
    }

    /// Whether the crowd is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }

    /// Adds an agent and computes its first path.
    ///
    /// Returns the new agent's id (its index), which stays valid until the crowd is replaced.
    ///
    /// # Errors
    /// Returns [`NavError::CrowdFull`] at [`MAX_AGENTS`], [`NavError::NotFinite`] for a bad radius
    /// or speed, and [`NavError::NoWalkableCell`] when the start or goal is off the mesh.
    pub fn add_agent(&mut self, params: AgentParams) -> Result<usize, NavError> {
        if self.agents.len() >= MAX_AGENTS {
            return Err(NavError::CrowdFull { max: MAX_AGENTS });
        }
        validate(params)?;
        let height = self
            .nav
            .height_at(params.position)
            .unwrap_or(params.position[1]);
        let mut agent = Agent::new(params, height);
        repath(&self.nav, &mut agent)?;
        self.agents.push(agent);
        Ok(self.agents.len() - 1)
    }

    /// Sends an agent to a new goal and recomputes its path.
    ///
    /// # Errors
    /// Returns [`NavError::NoAgent`] for an unknown id, and any pathfinding error from
    /// [`NavMesh::find_path`].
    pub fn set_goal(&mut self, id: usize, goal: [f32; 3]) -> Result<(), NavError> {
        let Self { nav, agents } = self;
        let agent = agents.get_mut(id).ok_or(NavError::NoAgent { id })?;
        agent.goal = goal;
        repath(nav, agent)
    }

    /// An agent by id.
    #[must_use]
    pub fn agent(&self, id: usize) -> Option<&Agent> {
        self.agents.get(id)
    }

    /// Every agent, in id order.
    pub fn agents(&self) -> impl Iterator<Item = &Agent> {
        self.agents.iter()
    }

    /// Advances the crowd by `dt` seconds.
    ///
    /// The step is clamped to 100 ms so a stalled frame cannot push agents through walls.
    pub fn step(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        if dt <= 0.0 || self.agents.is_empty() {
            return;
        }
        let Self { nav, agents } = self;
        let mut desired = desired_velocities(agents);
        apply_forces(agents, &mut desired);
        integrate(nav, agents, &desired, dt);
        relax(nav, agents);
        update_status(nav, agents);
    }
}

fn desired_velocities(agents: &mut [Agent]) -> Vec<Vec2> {
    agents
        .iter_mut()
        .map(|agent| {
            agent.advance_waypoint();
            agent.desired_velocity()
        })
        .collect()
}

fn apply_forces(agents: &[Agent], desired: &mut [Vec2]) {
    for i in 0..agents.len() {
        for j in (i + 1)..agents.len() {
            let (a, b) = (&agents[i], &agents[j]);
            let offset = b.position - a.position;
            let distance = offset.length();
            let radius_sum = a.radius + b.radius;
            let range = radius_sum * SEPARATION_RANGE;
            if distance <= 1.0e-6 || distance >= range {
                continue;
            }
            let direction = offset / distance;
            let separation = steering::separation_push(distance, radius_sum, range);
            if separation > 0.0 {
                let push = direction * (separation * 0.5);
                desired[i] -= push;
                desired[j] += push;
            }
            let closing = (desired[j] - desired[i]).dot(direction);
            if closing < 0.0 {
                let avoid = steering::avoidance_push(distance, -closing, AVOID_HORIZON);
                if avoid > 0.0 {
                    let push = direction * (avoid * 0.5);
                    desired[i] -= push;
                    desired[j] += push;
                }
            }
        }
    }
}

fn integrate(nav: &NavMesh, agents: &mut [Agent], desired: &[Vec2], dt: f32) {
    for (agent, wanted) in agents.iter_mut().zip(desired) {
        if agent.status == AgentStatus::Arrived {
            agent.velocity = Vec2::ZERO;
            continue;
        }
        let mut velocity = *wanted;
        let speed = velocity.length();
        if speed > agent.max_speed && speed > 1.0e-6 {
            velocity *= agent.max_speed / speed;
        }
        agent.velocity = velocity;
        let target = agent.position + velocity * dt;
        agent.position = steering::clamp_to_walkable(nav, agent.position, target);
    }
}

fn relax(nav: &NavMesh, agents: &mut [Agent]) {
    for _ in 0..RELAX_PASSES {
        for i in 0..agents.len() {
            for j in (i + 1)..agents.len() {
                let (head, tail) = agents.split_at_mut(j);
                let a = &mut head[i];
                let b = &mut tail[0];
                let offset = b.position - a.position;
                let distance = offset.length();
                let radius_sum = a.radius + b.radius;
                if distance >= radius_sum {
                    continue;
                }
                let direction = if distance > 1.0e-6 {
                    offset / distance
                } else {
                    Vec2::X
                };
                let push = direction * ((radius_sum - distance) * 0.5);
                a.position = steering::clamp_to_walkable(nav, a.position, a.position - push);
                b.position = steering::clamp_to_walkable(nav, b.position, b.position + push);
            }
        }
    }
}

fn update_status(nav: &NavMesh, agents: &mut [Agent]) {
    let arrive = nav.config().cell_size() * 0.75;
    for agent in agents.iter_mut() {
        let y = nav
            .height_at([agent.position.x, 0.0, agent.position.y])
            .unwrap_or(agent.height);
        agent.height = y;
        let goal = Vec2::new(agent.goal[0], agent.goal[2]);
        if goal.distance(agent.position) <= agent.radius + arrive {
            agent.status = AgentStatus::Arrived;
            agent.velocity = Vec2::ZERO;
            continue;
        }
        if agent.position.distance(agent.last_position) > 1.0e-4 {
            agent.stuck = 0;
            // A jam can clear; a wedged agent that starts moving again is no longer blocked.
            if agent.status == AgentStatus::Blocked && !agent.path.is_empty() {
                agent.status = AgentStatus::Moving;
            }
        } else {
            agent.stuck += 1;
            if agent.stuck >= STUCK_STEPS {
                agent.status = AgentStatus::Blocked;
            }
        }
        agent.last_position = agent.position;
    }
}
