//! One agent: its parameters, state, and path following.

use glam::Vec2;

use crate::error::NavError;
use crate::mesh::NavMesh;

/// Distance, in metres, at which a waypoint counts as reached.
pub(crate) const WAYPOINT_RADIUS: f32 = 0.15;

/// What an agent is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentStatus {
    /// Walking its path.
    Moving,
    /// At its goal.
    Arrived,
    /// It cannot make progress: no path, or wedged.
    Blocked,
}

/// Settings for one agent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AgentParams {
    /// Where the agent starts, in world space.
    pub position: [f32; 3],
    /// Body radius in metres.
    pub radius: f32,
    /// Top speed in metres per second.
    pub max_speed: f32,
    /// Where it is heading, in world space.
    pub goal: [f32; 3],
}

impl AgentParams {
    /// Parameters for an agent at `position` heading to `goal`.
    #[must_use]
    pub fn new(position: [f32; 3], radius: f32, max_speed: f32, goal: [f32; 3]) -> Self {
        Self {
            position,
            radius,
            max_speed,
            goal,
        }
    }
}

/// One agent in a [`Crowd`](crate::Crowd).
#[derive(Clone, Debug)]
pub struct Agent {
    pub(crate) position: Vec2,
    pub(crate) velocity: Vec2,
    pub(crate) height: f32,
    pub(crate) radius: f32,
    pub(crate) max_speed: f32,
    pub(crate) goal: [f32; 3],
    pub(crate) path: Vec<[f32; 3]>,
    pub(crate) waypoint: usize,
    pub(crate) status: AgentStatus,
    pub(crate) stuck: u32,
    pub(crate) last_position: Vec2,
}

impl Agent {
    pub(crate) fn new(params: AgentParams, height: f32) -> Self {
        let position = Vec2::new(params.position[0], params.position[2]);
        Self {
            position,
            velocity: Vec2::ZERO,
            height,
            radius: params.radius,
            max_speed: params.max_speed,
            goal: params.goal,
            path: Vec::new(),
            waypoint: 0,
            status: AgentStatus::Moving,
            stuck: 0,
            last_position: position,
        }
    }

    /// Current position, with its floor height.
    #[must_use]
    pub fn position(&self) -> [f32; 3] {
        [self.position.x, self.height, self.position.y]
    }

    /// Current velocity; the `y` component is always zero (the floor sets height).
    #[must_use]
    pub fn velocity(&self) -> [f32; 3] {
        [self.velocity.x, 0.0, self.velocity.y]
    }

    /// Body radius in metres.
    #[must_use]
    pub fn radius(&self) -> f32 {
        self.radius
    }

    /// Top speed in metres per second.
    #[must_use]
    pub fn max_speed(&self) -> f32 {
        self.max_speed
    }

    /// Current goal, in world space.
    #[must_use]
    pub fn goal(&self) -> [f32; 3] {
        self.goal
    }

    /// Current status.
    #[must_use]
    pub fn status(&self) -> AgentStatus {
        self.status
    }

    /// The route currently being followed.
    #[must_use]
    pub fn path(&self) -> &[[f32; 3]] {
        &self.path
    }

    /// Drops waypoints the agent has already reached.
    pub(crate) fn advance_waypoint(&mut self) {
        while self.waypoint < self.path.len() {
            let p = self.path[self.waypoint];
            let d = Vec2::new(p[0], p[2]).distance(self.position);
            if d <= WAYPOINT_RADIUS {
                self.waypoint += 1;
            } else {
                break;
            }
        }
    }

    /// Velocity the agent wants this step, before neighbours push it around.
    pub(crate) fn desired_velocity(&self) -> Vec2 {
        if self.status != AgentStatus::Moving {
            return Vec2::ZERO;
        }
        // Once the path's waypoints are exhausted, steer to the exact goal. The funnel can end on
        // a cell centre a little short of it, and a shove from the crowd must not strand the agent
        // outside the arrival radius with nothing left to walk to.
        let final_leg = self.waypoint >= self.path.len();
        let target = if final_leg {
            Vec2::new(self.goal[0], self.goal[2])
        } else {
            Vec2::new(self.path[self.waypoint][0], self.path[self.waypoint][2])
        };
        let to = target - self.position;
        let distance = to.length();
        if distance <= 1.0e-6 {
            return Vec2::ZERO;
        }
        let speed = if final_leg || self.waypoint + 1 == self.path.len() {
            self.max_speed.min(distance / 0.2)
        } else {
            self.max_speed
        };
        to / distance * speed
    }
}

/// Checks that an agent's numbers are finite and its radius and speed are non-negative.
pub(crate) fn validate(params: AgentParams) -> Result<(), NavError> {
    for (field, value) in [
        ("agent position x", params.position[0]),
        ("agent position y", params.position[1]),
        ("agent position z", params.position[2]),
        ("agent radius", params.radius),
        ("agent max speed", params.max_speed),
        ("agent goal x", params.goal[0]),
        ("agent goal y", params.goal[1]),
        ("agent goal z", params.goal[2]),
    ] {
        if !value.is_finite() {
            return Err(NavError::NotFinite { field, value });
        }
    }
    if params.radius < 0.0 {
        return Err(NavError::BadAgentRadius(params.radius));
    }
    if params.max_speed < 0.0 {
        return Err(NavError::BadSpeed(params.max_speed));
    }
    Ok(())
}

/// Recomputes an agent's route to its current goal.
pub(crate) fn repath(nav: &NavMesh, agent: &mut Agent) -> Result<(), NavError> {
    match nav.find_path(agent.position(), agent.goal) {
        Ok(path) => {
            agent.path = path.points().to_vec();
            agent.waypoint = 0;
            agent.status = AgentStatus::Moving;
            agent.stuck = 0;
            agent.last_position = agent.position;
            Ok(())
        }
        Err(error) => {
            agent.path.clear();
            agent.status = AgentStatus::Blocked;
            Err(error)
        }
    }
}
