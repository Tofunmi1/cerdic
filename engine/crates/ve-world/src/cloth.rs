//! Position-based dynamics for cloth, rope, hair and balloons: the XPBD solver (Macklin, Müller and Chentanez, "XPBD: Position-Based
//! Simulation of Compliant Constrained Dynamics", 2016) with small substeps.
//!
//! Particles have a position, a velocity and an inverse mass (0 pins a particle). Constraints pull them toward distances and a volume;
//! each has a compliance (0 is rigid, larger is softer and independent of step size and iteration count). Colliders push particles out
//! and friction slows what slides on them. Everything runs in a fixed order and uses `+ - * /` and `sqrt` only, so a simulation is the
//! same on every platform. Self-collision is not modelled.

mod build;
mod collide;

pub use build::{balloon, cloth_from_mesh, rope};
pub use collide::{Collider, SdfGrid};

/// A point or vector in metres.
pub type V3 = [f32; 3];

/// The most particles one solver holds.
pub const MAX_PARTICLES: usize = 200_000;

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale(a: V3, s: f32) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// How a simulation runs.
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// Acceleration in metres per second squared.
    pub gravity: V3,
    /// Steps per frame; more is stiffer and steadier.
    pub substeps: u32,
    /// Constraint passes per substep.
    pub iterations: u32,
    /// Velocity lost per second, 0 to about 5.
    pub damping: f32,
    /// How far from a collider's surface particles are kept, in metres (the cloth's own thickness).
    pub thickness: f32,
    /// Share of sliding speed lost on contact, 0 to 1.
    pub friction: f32,
    /// Wind velocity in metres per second.
    pub wind: V3,
    /// How hard air pushes on the surface (0 for none).
    pub drag: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            gravity: [0.0, -9.81, 0.0],
            substeps: 8,
            iterations: 1,
            damping: 0.4,
            thickness: 0.005,
            friction: 0.4,
            wind: [0.0; 3],
            drag: 0.0,
        }
    }
}

/// A pull toward a rest distance between two particles.
#[derive(Clone, Copy, Debug)]
struct Distance {
    a: u32,
    b: u32,
    rest: f32,
    compliance: f32,
}

/// A closed surface that keeps (or, with pressure, changes) its enclosed volume.
#[derive(Clone, Debug)]
struct Balloon {
    triangles: Vec<[u32; 3]>,
    rest_volume: f32,
    compliance: f32,
}

/// The simulation: particles, constraints and colliders.
#[derive(Clone, Debug)]
pub struct Solver {
    /// Particle positions.
    pub pos: Vec<V3>,
    /// Particle velocities.
    pub vel: Vec<V3>,
    /// Inverse masses; 0 pins a particle in place.
    pub inv_mass: Vec<f32>,
    /// Shapes particles are pushed out of.
    pub colliders: Vec<Collider>,
    /// How the simulation runs.
    pub settings: Settings,
    prev: Vec<V3>,
    distances: Vec<Distance>,
    lambdas: Vec<f32>,
    triangles: Vec<[u32; 3]>,
    balloons: Vec<Balloon>,
    balloon_lambdas: Vec<f32>,
}

impl Solver {
    /// Particles at `positions` with the given inverse masses.
    #[must_use]
    pub fn new(positions: Vec<V3>, inv_mass: Vec<f32>) -> Self {
        let n = positions.len();
        Self {
            vel: vec![[0.0; 3]; n],
            prev: positions.clone(),
            pos: positions,
            inv_mass,
            colliders: Vec::new(),
            settings: Settings::default(),
            distances: Vec::new(),
            lambdas: Vec::new(),
            triangles: Vec::new(),
            balloons: Vec::new(),
            balloon_lambdas: Vec::new(),
        }
    }

    /// Keeps `a` and `b` at their rest distance `rest`, softly by `compliance`.
    pub fn add_distance(&mut self, a: u32, b: u32, rest: f32, compliance: f32) {
        self.distances.push(Distance {
            a,
            b,
            rest,
            compliance,
        });
        self.lambdas.push(0.0);
    }

    /// The triangles the wind pushes on.
    pub fn set_triangles(&mut self, triangles: Vec<[u32; 3]>) {
        self.triangles = triangles;
    }

    /// Keeps the closed surface `triangles` at `pressure` times its present volume (1 holds it, above 1 inflates it).
    pub fn add_balloon(&mut self, triangles: Vec<[u32; 3]>, pressure: f32, compliance: f32) {
        let rest_volume = volume(&self.pos, &triangles) * pressure;
        self.balloons.push(Balloon {
            triangles,
            rest_volume,
            compliance,
        });
        self.balloon_lambdas.push(0.0);
    }

    /// Pins particle `i` where it is.
    pub fn pin(&mut self, i: usize) {
        self.inv_mass[i] = 0.0;
        self.vel[i] = [0.0; 3];
    }

    /// Moves a pinned particle (a root following an animated bone) without giving it a velocity.
    pub fn move_pin(&mut self, i: usize, to: V3) {
        self.pos[i] = to;
        self.prev[i] = to;
    }

    /// The enclosed volume of triangles `triangles`, in cubic metres.
    #[must_use]
    pub fn volume_of(&self, triangles: &[[u32; 3]]) -> f32 {
        volume(&self.pos, triangles)
    }

    /// The largest stretch of any distance constraint, as its length over its rest length (1 is relaxed).
    #[must_use]
    pub fn max_stretch(&self) -> f32 {
        self.distances
            .iter()
            .filter(|c| c.rest > 1e-9)
            .map(|c| {
                let d = sub(self.pos[c.a as usize], self.pos[c.b as usize]);
                dot(d, d).sqrt() / c.rest
            })
            .fold(0.0, f32::max)
    }

    /// Advances by `dt` seconds. Returns false when a position became non-finite; the state is then unusable and the solver must be discarded.
    pub fn step(&mut self, dt: f32) -> bool {
        let substeps = self.settings.substeps.max(1);
        let h = dt / substeps as f32;
        for _ in 0..substeps {
            self.substep(h);
        }
        self.pos.iter().all(|p| p.iter().all(|c| c.is_finite()))
    }

    fn substep(&mut self, h: f32) {
        let s = self.settings;
        self.apply_wind(h);
        for i in 0..self.pos.len() {
            self.prev[i] = self.pos[i];
            if self.inv_mass[i] > 0.0 {
                self.vel[i] = add(self.vel[i], scale(s.gravity, h));
                self.pos[i] = add(self.pos[i], scale(self.vel[i], h));
            }
        }
        self.lambdas.fill(0.0);
        self.balloon_lambdas.fill(0.0);
        for _ in 0..s.iterations.max(1) {
            self.solve_distances(h);
            self.solve_balloons(h);
        }
        let contacts = self.collide();
        for i in 0..self.pos.len() {
            if self.inv_mass[i] <= 0.0 {
                continue;
            }
            let mut v = scale(sub(self.pos[i], self.prev[i]), 1.0 / h);
            if let Some(n) = contacts[i] {
                let normal_speed = dot(v, n);
                let tangent = sub(v, scale(n, normal_speed));
                v = add(
                    scale(n, normal_speed.max(0.0)),
                    scale(tangent, 1.0 - s.friction.clamp(0.0, 1.0)),
                );
            }
            self.vel[i] = scale(v, 1.0 - (s.damping * h).clamp(0.0, 1.0));
        }
    }

    fn apply_wind(&mut self, h: f32) {
        let s = self.settings;
        if s.drag <= 0.0 {
            return;
        }
        for t in &self.triangles {
            let [a, b, c] = t.map(|i| i as usize);
            let normal = cross(sub(self.pos[b], self.pos[a]), sub(self.pos[c], self.pos[a]));
            let double_area = dot(normal, normal).sqrt();
            if double_area < 1e-12 {
                continue;
            }
            let unit = scale(normal, 1.0 / double_area);
            let average = scale(add(add(self.vel[a], self.vel[b]), self.vel[c]), 1.0 / 3.0);
            let relative = sub(s.wind, average);
            // Air pushes on a surface in proportion to its area and to how squarely the relative wind meets it.
            let force = scale(unit, s.drag * 0.5 * double_area * dot(unit, relative) / 3.0);
            for i in [a, b, c] {
                if self.inv_mass[i] > 0.0 {
                    self.vel[i] = add(self.vel[i], scale(force, self.inv_mass[i] * h));
                }
            }
        }
    }

    fn solve_distances(&mut self, h: f32) {
        let inverse_step_squared = 1.0 / (h * h);
        for (k, c) in self.distances.iter().enumerate() {
            let (a, b) = (c.a as usize, c.b as usize);
            let (wa, wb) = (self.inv_mass[a], self.inv_mass[b]);
            let weight = wa + wb;
            if weight <= 0.0 {
                continue;
            }
            let d = sub(self.pos[a], self.pos[b]);
            let length = dot(d, d).sqrt();
            if length < 1e-9 {
                continue;
            }
            let alpha = c.compliance * inverse_step_squared;
            let delta = (-(length - c.rest) - alpha * self.lambdas[k]) / (weight + alpha);
            self.lambdas[k] += delta;
            let correction = scale(d, delta / length);
            self.pos[a] = add(self.pos[a], scale(correction, wa));
            self.pos[b] = sub(self.pos[b], scale(correction, wb));
        }
    }

    fn solve_balloons(&mut self, h: f32) {
        let inverse_step_squared = 1.0 / (h * h);
        for (k, balloon) in self.balloons.iter().enumerate() {
            let mut gradient = vec![[0.0_f32; 3]; self.pos.len()];
            let mut enclosed = 0.0;
            for t in &balloon.triangles {
                let [a, b, c] = t.map(|i| i as usize);
                let (pa, pb, pc) = (self.pos[a], self.pos[b], self.pos[c]);
                enclosed += dot(pa, cross(pb, pc)) / 6.0;
                gradient[a] = add(gradient[a], scale(cross(pb, pc), 1.0 / 6.0));
                gradient[b] = add(gradient[b], scale(cross(pc, pa), 1.0 / 6.0));
                gradient[c] = add(gradient[c], scale(cross(pa, pb), 1.0 / 6.0));
            }
            let weight: f32 = gradient
                .iter()
                .zip(&self.inv_mass)
                .map(|(g, w)| w * dot(*g, *g))
                .sum();
            if weight <= 0.0 {
                continue;
            }
            let alpha = balloon.compliance * inverse_step_squared;
            let delta = (-(enclosed - balloon.rest_volume) - alpha * self.balloon_lambdas[k])
                / (weight + alpha);
            self.balloon_lambdas[k] += delta;
            for (i, g) in gradient.iter().enumerate() {
                if self.inv_mass[i] > 0.0 {
                    self.pos[i] = add(self.pos[i], scale(*g, self.inv_mass[i] * delta));
                }
            }
        }
    }

    /// Pushes particles out of every collider; returns, per particle, the direction of the deepest contact.
    fn collide(&mut self) -> Vec<Option<V3>> {
        let margin = self.settings.thickness;
        let mut contacts = vec![None; self.pos.len()];
        for i in 0..self.pos.len() {
            if self.inv_mass[i] <= 0.0 {
                continue;
            }
            for collider in &self.colliders {
                let (distance, normal) = collider.distance(self.pos[i]);
                if distance < margin {
                    self.pos[i] = add(self.pos[i], scale(normal, margin - distance));
                    contacts[i] = Some(normal);
                }
            }
        }
        contacts
    }
}

fn volume(pos: &[V3], triangles: &[[u32; 3]]) -> f32 {
    triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| pos[i as usize]);
            dot(a, cross(b, c)) / 6.0
        })
        .sum()
}
