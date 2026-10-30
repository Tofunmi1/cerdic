//! Bounded 3D position-based liquid reference solver.
//!
//! This is a CPU correctness/reference path, not the production high-particle-count runtime.
use std::collections::BTreeMap;
#[path = "volume/math.rs"]
mod math;
mod types;
use math::{
    add, bound_magnitude, cell, collide, distance_sq, dot, grad_spiky, inside, mul, poly6, sub,
    validate_options,
};
pub use types::{Error, Options, Particle};

/// Maximum particles for this deterministic CPU reference solver.
pub const MAX_PARTICLES: usize = 4096;
const MAX_STEP: f32 = 1.0 / 30.0;
const MAX_GRID_COORDINATE: f32 = 2_000_000_000.0;

/// A finite, bounded, three-dimensional particle liquid.
pub struct Volume {
    particles: Vec<Particle>,
    options: Options,
    floor: Option<FloorGrid>,
    time: f64,
}

#[derive(Clone)]
struct FloorGrid {
    origin: [f32; 2],
    cell: f32,
    width: usize,
    depth: usize,
    heights: Vec<f32>,
}

impl Volume {
    /// Creates a volume from explicit particle initial conditions.
    ///
    /// # Errors
    /// Returns an error for invalid parameters, an empty/oversized set, or a particle outside bounds.
    pub fn new(particles: Vec<Particle>, options: Options) -> Result<Self, Error> {
        validate_options(options)?;
        if particles.is_empty() {
            return Err(Error::Empty);
        }
        if particles.len() > MAX_PARTICLES {
            return Err(Error::ParticleLimit);
        }
        for (index, particle) in particles.iter().enumerate() {
            if !particle.position.iter().all(|v| v.is_finite())
                || !particle.velocity.iter().all(|v| v.is_finite())
            {
                return Err(Error::Parameter("particle state"));
            }
            if !inside(particle.position, options) {
                return Err(Error::Outside(index));
            }
        }
        Ok(Self {
            particles,
            options,
            floor: None,
            time: 0.0,
        })
    }

    /// Creates a regular particle fill in the 3D box; spacing must be smaller than the kernel radius.
    ///
    /// # Errors
    /// Returns an error for invalid bounds, spacing, or a fill that exceeds [`MAX_PARTICLES`].
    pub fn filled_box(options: Options, spacing: f32) -> Result<Self, Error> {
        validate_options(options)?;
        if !spacing.is_finite() || spacing <= 0.0 || spacing >= options.smoothing_radius {
            return Err(Error::Parameter("particle spacing"));
        }
        let mut particles = Vec::new();
        let mut z = options.min[2] + spacing * 0.5;
        while z < options.max[2] {
            let mut y = options.min[1] + spacing * 0.5;
            while y < options.max[1] {
                let mut x = options.min[0] + spacing * 0.5;
                while x < options.max[0] {
                    if particles.len() == MAX_PARTICLES {
                        return Err(Error::ParticleLimit);
                    }
                    particles.push(Particle {
                        position: [x, y, z],
                        velocity: [0.0; 3],
                    });
                    x += spacing;
                }
                y += spacing;
            }
            z += spacing;
        }
        Self::new(particles, options)
    }

    /// Advances with PBF density projection and closed-box collisions.
    ///
    /// # Errors
    /// Returns [`Error::Step`] when `dt` is outside the finite supported interval.
    pub fn step(&mut self, dt: f32) -> Result<(), Error> {
        if !dt.is_finite() || dt <= 0.0 || dt > MAX_STEP {
            return Err(Error::Step);
        }
        let old: Vec<_> = self.particles.iter().map(|p| p.position).collect();
        let options = self.options;
        let floor = self.floor.clone();
        for particle in &mut self.particles {
            for axis in 0..3 {
                particle.velocity[axis] += self.options.gravity[axis] * dt;
                particle.position[axis] += particle.velocity[axis] * dt;
            }
            collide(particle, options, floor.as_ref());
        }

        for _ in 0..self.options.iterations {
            self.project_density();
        }

        let decay = (-self.options.damping * dt).exp();
        for (particle, previous) in self.particles.iter_mut().zip(old) {
            for (axis, previous_axis) in previous.iter().enumerate() {
                particle.velocity[axis] = (particle.position[axis] - previous_axis) / dt * decay;
            }
        }
        self.time += f64::from(dt);
        Ok(())
    }

    /// Current particle states in deterministic insertion order.
    #[must_use]
    pub fn particles(&self) -> &[Particle] {
        &self.particles
    }

    /// Elapsed simulated time in seconds.
    #[must_use]
    pub fn time(&self) -> f64 {
        self.time
    }

    /// Approximate liquid volume represented by the fixed particle masses.
    #[must_use]
    pub fn particle_volume(&self) -> f64 {
        let count = u32::try_from(self.particles.len()).map_or(0.0, f64::from);
        f64::from(self.options.particle_mass) * count / f64::from(self.options.rest_density)
    }

    /// Axis-aligned bounds of the current particle positions.
    #[must_use]
    pub fn particle_bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let first = self.particles.first()?;
        let mut min = first.position;
        let mut max = first.position;
        for particle in &self.particles[1..] {
            for axis in 0..3 {
                min[axis] = min[axis].min(particle.position[axis]);
                max[axis] = max[axis].max(particle.position[axis]);
            }
        }
        Some((min, max))
    }

    /// Applies a compact radial velocity impulse in world space.
    pub fn impulse(&mut self, center: [f32; 3], radius: f32, velocity: [f32; 3]) -> usize {
        self.apply_impulse(center, radius, velocity, false)
    }

    /// Applies a surface disturbance through the liquid column at an XZ point.
    pub fn impulse_column(&mut self, center: [f32; 3], radius: f32, velocity: [f32; 3]) -> usize {
        self.apply_impulse(center, radius, velocity, true)
    }

    fn apply_impulse(
        &mut self,
        center: [f32; 3],
        radius: f32,
        velocity: [f32; 3],
        column: bool,
    ) -> usize {
        if !center.iter().chain(&velocity).all(|v| v.is_finite())
            || !radius.is_finite()
            || radius <= 0.0
        {
            return 0;
        }
        let radius = if column {
            radius.max(self.options.smoothing_radius)
        } else {
            radius
        };
        let radius_sq = radius * radius;
        let nearest = column
            .then(|| {
                self.particles
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| {
                        let distance = |p: &Particle| {
                            (center[0] - p.position[0]).powi(2)
                                + (center[2] - p.position[2]).powi(2)
                        };
                        distance(a).total_cmp(&distance(b))
                    })
                    .map(|(index, _)| index)
            })
            .flatten();
        let mut changed = 0;
        for (index, particle) in self.particles.iter_mut().enumerate() {
            let delta = sub(center, particle.position);
            let distance = if column {
                delta[0] * delta[0] + delta[2] * delta[2]
            } else {
                dot(delta, delta)
            };
            if distance <= radius_sq || nearest == Some(index) {
                let weight = if distance <= radius_sq {
                    1.0 - distance.sqrt() / radius
                } else {
                    1.0
                };
                for (axis, impulse) in velocity.iter().enumerate() {
                    particle.velocity[axis] += impulse * weight;
                }
                changed += 1;
            }
        }
        changed
    }

    /// Adds a sampled terrain bed for a basin fill. Dry cells use the waterline as their floor,
    /// keeping the simulated volume from escaping through the shoreline.
    pub fn set_floor_grid(
        &mut self,
        origin: [f32; 2],
        cell: f32,
        width: usize,
        depth: usize,
        heights: Vec<f32>,
    ) -> bool {
        if width == 0
            || depth == 0
            || width.checked_mul(depth) != Some(heights.len())
            || !origin.iter().all(|v| v.is_finite())
            || !cell.is_finite()
            || cell <= 0.0
            || heights.iter().any(|y| !y.is_finite())
        {
            return false;
        }
        self.floor = Some(FloorGrid {
            origin,
            cell,
            width,
            depth,
            heights,
        });
        let options = self.options;
        let floor = self.floor.clone();
        let mut particles = std::mem::take(&mut self.particles);
        for particle in &mut particles {
            collide(particle, options, floor.as_ref());
        }
        self.particles = particles;
        true
    }

    fn project_density(&mut self) {
        let neighbors = self.neighbors();
        let densities: Vec<_> = neighbors
            .iter()
            .enumerate()
            .map(|(i, near)| {
                near.iter()
                    .map(|&j| {
                        self.options.particle_mass
                            * poly6(
                                distance_sq(self.particles[i].position, self.particles[j].position),
                                self.options.smoothing_radius,
                            )
                    })
                    .sum::<f32>()
            })
            .collect();
        let scale = self.options.particle_mass / self.options.rest_density;
        let lambdas: Vec<_> = neighbors
            .iter()
            .enumerate()
            .map(|(i, near)| {
                let mut gradient_i = [0.0; 3];
                let mut sum = 0.0;
                for &j in near {
                    if i == j {
                        continue;
                    }
                    let grad = mul(
                        grad_spiky(
                            sub(self.particles[i].position, self.particles[j].position),
                            self.options.smoothing_radius,
                        ),
                        scale,
                    );
                    gradient_i = add(gradient_i, grad);
                    sum += dot(grad, grad);
                }
                sum += dot(gradient_i, gradient_i);
                let constraint = densities[i] / self.options.rest_density - 1.0;
                -constraint / (sum + 1e-6)
            })
            .collect();
        let corrections: Vec<_> = neighbors
            .iter()
            .enumerate()
            .map(|(i, near)| {
                let correction = near.iter().filter(|&&j| i != j).fold([0.0; 3], |sum, &j| {
                    let grad = mul(
                        grad_spiky(
                            sub(self.particles[i].position, self.particles[j].position),
                            self.options.smoothing_radius,
                        ),
                        scale,
                    );
                    add(sum, mul(grad, lambdas[i] + lambdas[j]))
                });
                bound_magnitude(correction, self.options.smoothing_radius * 0.1)
            })
            .collect();
        let options = self.options;
        let floor = self.floor.clone();
        for (particle, correction) in self.particles.iter_mut().zip(corrections) {
            particle.position = add(particle.position, correction);
            collide(particle, options, floor.as_ref());
        }
    }

    fn neighbors(&self) -> Vec<Vec<usize>> {
        let radius = self.options.smoothing_radius;
        let mut cells: BTreeMap<[i32; 3], Vec<usize>> = BTreeMap::new();
        for (i, particle) in self.particles.iter().enumerate() {
            cells
                .entry(cell(particle.position, self.options.min, radius))
                .or_default()
                .push(i);
        }
        self.particles
            .iter()
            .enumerate()
            .map(|(self_id, particle)| {
                let at = cell(particle.position, self.options.min, radius);
                let mut found = Vec::new();
                for z in -1..=1 {
                    for y in -1..=1 {
                        for x in -1..=1 {
                            if let Some(items) = cells.get(&[at[0] + x, at[1] + y, at[2] + z]) {
                                found.extend(items.iter().copied());
                            }
                        }
                    }
                }
                found.sort_unstable();
                debug_assert!(found.contains(&self_id));
                found
            })
            .collect()
    }
}
