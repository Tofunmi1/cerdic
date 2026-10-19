//! Small goal pickers for game AI, built on top of the nav mesh.
//!
//! Each behaviour is a [`Goal`]: call [`Goal::goal`] once per decision (or per frame) with the
//! agent's position and feed the result to [`crate::Crowd::set_goal`]. They are pure state machines
//! with no clock of their own, so they are deterministic and easy to test.

use glam::Vec2;

use crate::mesh::NavMesh;

/// Something that can choose the next goal for an agent.
pub trait Goal {
    /// Returns a world-space goal, or `None` when this behaviour has nothing to suggest.
    fn goal(&mut self, nav: &NavMesh, position: [f32; 3]) -> Option<[f32; 3]>;
}

/// Whether `a` can see `b` without crossing blocked ground.
#[must_use]
pub fn line_of_sight(nav: &NavMesh, a: [f32; 3], b: [f32; 3]) -> bool {
    nav.line_of_sight(a, b)
}

/// Wanders to random nearby walkable points.
pub struct Wander {
    rng: Rng,
    distance: f32,
}

impl Wander {
    /// A wanderer of radius 4 metres, driven by `seed`.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            rng: Rng::new(seed),
            distance: 4.0,
        }
    }

    /// A wanderer that picks goals `distance` metres away.
    #[must_use]
    pub fn with_distance(seed: u64, distance: f32) -> Self {
        Self {
            rng: Rng::new(seed),
            distance,
        }
    }
}

impl Goal for Wander {
    fn goal(&mut self, nav: &NavMesh, position: [f32; 3]) -> Option<[f32; 3]> {
        let angle = self.rng.unit() * std::f32::consts::TAU;
        let target = [
            position[0] + angle.cos() * self.distance,
            position[1],
            position[2] + angle.sin() * self.distance,
        ];
        nav.nearest_walkable(target)
    }
}

/// Walks a fixed loop of points, in order.
pub struct Patrol {
    points: Vec<[f32; 3]>,
    index: usize,
}

impl Patrol {
    /// A patrol around `points`; an empty list makes [`Goal::goal`] return `None`.
    #[must_use]
    pub fn new(points: Vec<[f32; 3]>) -> Self {
        Self { points, index: 0 }
    }
}

impl Goal for Patrol {
    fn goal(&mut self, nav: &NavMesh, position: [f32; 3]) -> Option<[f32; 3]> {
        if self.points.is_empty() {
            return None;
        }
        let current = self.points[self.index];
        if Vec2::new(current[0], current[2]).distance(Vec2::new(position[0], position[2])) < 0.5 {
            self.index = (self.index + 1) % self.points.len();
        }
        let target = self.points[self.index];
        Some(nav.nearest_walkable(target).unwrap_or(target))
    }
}

/// Heads straight for a moving target.
pub struct Chase {
    target: [f32; 3],
}

impl Chase {
    /// A chase behaviour for `target`.
    #[must_use]
    pub fn new(target: [f32; 3]) -> Self {
        Self { target }
    }

    /// Moves the target, for example to the player's current position.
    pub fn set_target(&mut self, target: [f32; 3]) {
        self.target = target;
    }
}

impl Goal for Chase {
    fn goal(&mut self, nav: &NavMesh, _position: [f32; 3]) -> Option<[f32; 3]> {
        nav.nearest_walkable(self.target)
    }
}

/// Runs away from a threat.
pub struct Flee {
    threat: [f32; 3],
    distance: f32,
}

impl Flee {
    /// A flee behaviour that aims `distance` metres away from `threat`.
    #[must_use]
    pub fn new(threat: [f32; 3], distance: f32) -> Self {
        Self { threat, distance }
    }

    /// Moves the threat.
    pub fn set_threat(&mut self, threat: [f32; 3]) {
        self.threat = threat;
    }
}

impl Goal for Flee {
    fn goal(&mut self, nav: &NavMesh, position: [f32; 3]) -> Option<[f32; 3]> {
        let away = Vec2::new(position[0] - self.threat[0], position[2] - self.threat[2]);
        let distance = away.length();
        let direction = if distance > 1.0e-4 {
            away / distance
        } else {
            Vec2::X
        };
        let target = [
            position[0] + direction.x * self.distance,
            position[1],
            position[2] + direction.y * self.distance,
        ];
        nav.nearest_walkable(target)
    }
}

/// A tiny deterministic PRNG so wander looks random but replays identically.
struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: u64) -> Self {
        // A zero state would make splitmix64 a constant; mix the seed first.
        Self {
            state: seed ^ 0x9E37_79B9_7F4A_7C15,
        }
    }

    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A value in `[0, 1)`.
    fn unit(&mut self) -> f32 {
        let bits = self.next() >> 40;
        bits as f32 / (1_u32 << 24) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wander_with_the_same_seed_repeats() {
        let mut first = Rng::new(7);
        let mut second = Rng::new(7);
        for _ in 0..10 {
            assert_eq!(first.unit().to_bits(), second.unit().to_bits());
        }
    }

    #[test]
    fn different_seeds_pick_different_goals() {
        let mut first = Rng::new(1);
        let mut second = Rng::new(2);
        let a: Vec<u32> = (0..8).map(|_| first.unit().to_bits()).collect();
        let b: Vec<u32> = (0..8).map(|_| second.unit().to_bits()).collect();
        assert_ne!(a, b);
    }
}
