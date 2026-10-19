use crate::spec::{Colour, Curve, EmitterSpec, Vec3};
use glam::{Mat4, Vec3 as GVec3};

/// Fixed simulation interval, in seconds.
pub const STEP: f32 = 1.0 / 120.0;
const MAX_FRAME: f32 = 10.0;
const STEP64: f64 = 1.0 / 120.0;
/// A simulated particle. Values are stored in world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    /// World position.
    pub position: Vec3,
    /// World velocity.
    pub velocity: Vec3,
    /// Current age in seconds.
    pub age: f32,
    /// Lifetime in seconds.
    pub lifetime: f32,
    /// Rotation in radians.
    pub rotation: f32,
    /// Spin in radians per second.
    pub spin: f32,
    /// Whether the particle is alive.
    pub alive: bool,
}
/// Render-ready per-particle billboard values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParticleInstance {
    /// World position.
    pub position: Vec3,
    /// Current size.
    pub size: f32,
    /// Current rotation in radians.
    pub rotation: f32,
    /// Current RGBA colour.
    pub colour: Colour,
    /// Normalized lifetime age.
    pub age: f32,
}
/// Fixed-step particle emitter and simulation.
pub struct ParticleSystem {
    spec: EmitterSpec,
    particles: Vec<Particle>,
    rng: u64,
    accumulator: f64,
    spawn_credit: f32,
    queued: usize,
    transform: Mat4,
}
impl ParticleSystem {
    /// Construct a system from a valid spec. Invalid specs produce an empty system; use `EmitterSpec::validate` at input boundaries.
    #[must_use]
    pub fn new(spec: EmitterSpec) -> Self {
        let mut s = Self {
            rng: spec.seed,
            particles: Vec::with_capacity(spec.max_particles),
            queued: spec
                .bursts
                .iter()
                .map(|n| usize::try_from(*n).unwrap_or(usize::MAX))
                .fold(0, usize::saturating_add),
            spec,
            accumulator: 0.0,
            spawn_credit: 0.0,
            transform: Mat4::IDENTITY,
        };
        if s.spec.validate().is_err() {
            s.queued = 0;
        }
        s
    }
    /// Queue a one-shot burst; births are bounded by the configured hard cap.
    pub fn burst(&mut self, count: usize) {
        self.queued = self.queued.saturating_add(count);
    }
    /// Advance the simulation; frame time above 10 seconds is clamped to prevent unbounded catch-up work.
    pub fn step(&mut self, dt: f32, emitter_to_world: Mat4) {
        if !dt.is_finite() || dt <= 0.0 || self.spec.validate().is_err() {
            return;
        }
        self.transform = emitter_to_world;
        self.accumulator += f64::from(dt.min(MAX_FRAME));
        while self.accumulator + 1.0e-7 >= STEP64 {
            self.tick(STEP);
            self.accumulator -= STEP64;
            if self.accumulator < 0.0 {
                self.accumulator = 0.0;
            }
        }
    }
    /// Read live particles in stable allocation order.
    #[must_use]
    pub fn particles(&self) -> &[Particle] {
        &self.particles
    }
    /// Build render instances in stable allocation order.
    #[must_use]
    pub fn instances(&self) -> Vec<ParticleInstance> {
        self.particles
            .iter()
            .filter(|p| p.alive)
            .map(|p| self.instance(p))
            .collect()
    }
    /// Build render instances sorted farthest from the camera first.
    #[must_use]
    pub fn sorted_instances(&self, camera_pos: Vec3) -> Vec<ParticleInstance> {
        let mut v = self.instances();
        v.sort_by(|a, b| {
            distance2(b.position, camera_pos).total_cmp(&distance2(a.position, camera_pos))
        });
        v
    }
    fn instance(&self, p: &Particle) -> ParticleInstance {
        let age = (p.age / p.lifetime).clamp(0.0, 1.0);
        ParticleInstance {
            position: p.position,
            size: sample_f32(&self.spec.size, age),
            rotation: p.rotation,
            colour: sample_colour(&self.spec.colour, age),
            age,
        }
    }
    fn tick(&mut self, dt: f32) {
        self.spawn_credit += self.spec.spawn_rate * dt;
        let mut births = self.queued.min(self.spec.max_particles);
        self.queued = 0;
        while self.spawn_credit >= 1.0 && births < self.spec.max_particles {
            self.spawn_credit -= 1.0;
            births += 1;
        }
        if self.spawn_credit >= 1.0 {
            self.spawn_credit = self.spawn_credit.fract();
        }
        for _ in 0..births {
            self.spawn();
        }
        for p in &mut self.particles {
            if !p.alive {
                continue;
            }
            let age = (p.age / p.lifetime).clamp(0.0, 1.0);
            let noise = if self.spec.turbulence == 0.0 {
                Vec3::default()
            } else {
                Vec3 {
                    x: hash_float(self.rng, p.age * self.spec.turbulence_scale, 0),
                    y: hash_float(self.rng, p.age * self.spec.turbulence_scale, 1),
                    z: hash_float(self.rng, p.age * self.spec.turbulence_scale, 2),
                }
                .mul(self.spec.turbulence)
            };
            p.velocity = p.velocity.add(self.spec.gravity.add(noise).mul(dt));
            p.velocity = p.velocity.mul(1.0 / (1.0 + self.spec.drag * dt));
            p.position = p.position.add(p.velocity.mul(dt));
            p.rotation += p.spin * dt;
            p.age += dt;
            if let Some(g) = self.spec.ground
                && p.position.y < g.height
            {
                if g.kill_on_hit {
                    p.alive = false;
                } else {
                    p.position.y = g.height;
                    p.velocity.y = p.velocity.y.abs() * g.bounce;
                }
            }
            if age >= 1.0 || p.age + 1.0e-6 >= p.lifetime {
                p.age = p.lifetime;
                p.alive = false;
            }
        }
    }
    fn spawn(&mut self) {
        let idx = self.particles.iter().position(|p| !p.alive);
        if idx.is_none() && self.particles.len() >= self.spec.max_particles {
            return;
        }
        let rand = |s: &mut Self| s.random();
        let life = mix(self.spec.lifetime[0], self.spec.lifetime[1], rand(self));
        let speed = mix(self.spec.speed[0], self.spec.speed[1], rand(self));
        let spin = mix(self.spec.spin[0], self.spec.spin[1], rand(self));
        let mut pos = sample_shape(self.spec.shape, self);
        let dir = sample_direction(self.spec.direction, self.spec.cone_angle, self);
        pos = self
            .transform
            .transform_point3(GVec3::new(pos.x, pos.y, pos.z))
            .into();
        let velocity = self
            .transform
            .transform_vector3(GVec3::new(dir.x * speed, dir.y * speed, dir.z * speed))
            .into();
        let p = Particle {
            position: pos,
            velocity,
            age: 0.0,
            lifetime: life,
            rotation: 0.0,
            spin,
            alive: true,
        };
        if let Some(i) = idx {
            self.particles[i] = p;
        } else {
            self.particles.push(p);
        }
    }
    fn random(&mut self) -> f32 {
        self.rng = self.rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        f32::from_bits(0x3F80_0000 | u32::try_from(z >> 41).unwrap_or_default()) - 1.0
    }
}
fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
fn sample_f32(c: &Curve<f32>, age: f32) -> f32 {
    c.sample(age, mix)
}
fn sample_colour(c: &Curve<Colour>, age: f32) -> Colour {
    c.sample(age, |a, b, t| Colour {
        r: mix(a.r, b.r, t),
        g: mix(a.g, b.g, t),
        b: mix(a.b, b.b, t),
        a: mix(a.a, b.a, t),
    })
}
fn distance2(first: Vec3, second: Vec3) -> f32 {
    let dx = first.x - second.x;
    let dy = first.y - second.y;
    let dz = first.z - second.z;
    dx * dx + dy * dy + dz * dz
}
fn hash_float(seed: u64, t: f32, axis: u64) -> f32 {
    let bits = u64::from(t.to_bits())
        .wrapping_add(seed)
        .wrapping_add(axis * 0x9E37_79B9);
    let v = bits.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    (f32::from_bits(0x3F80_0000 | u32::try_from(v >> 41).unwrap_or_default()) - 1.0) * 2.0 - 1.0
}
fn sample_shape(shape: crate::Shape, s: &mut ParticleSystem) -> Vec3 {
    match shape {
        crate::Shape::Point => Vec3::default(),
        crate::Shape::Sphere { radius } => {
            let v = Vec3 {
                x: s.random() * 2.0 - 1.0,
                y: s.random() * 2.0 - 1.0,
                z: s.random() * 2.0 - 1.0,
            };
            v.normalized().mul(radius * s.random())
        }
        crate::Shape::Box { half_extents: h } => Vec3 {
            x: (s.random() * 2.0 - 1.0) * h.x,
            y: (s.random() * 2.0 - 1.0) * h.y,
            z: (s.random() * 2.0 - 1.0) * h.z,
        },
        crate::Shape::Disc { radius } => {
            let a = s.random() * 2.0 - 1.0;
            let b = s.random() * 2.0 - 1.0;
            Vec3 {
                x: a * radius,
                y: 0.0,
                z: b * radius,
            }
        }
        crate::Shape::Line { length } => Vec3 {
            x: 0.0,
            y: (s.random() * 2.0 - 1.0) * length * 0.5,
            z: 0.0,
        },
    }
}
fn sample_direction(direction: Vec3, cone: f32, s: &mut ParticleSystem) -> Vec3 {
    let base = direction.normalized();
    let reference = if base.y.abs() < 0.9 {
        Vec3 {
            x: 0.0,
            y: 1.0,
            z: 0.0,
        }
    } else {
        Vec3 {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        }
    };
    let tangent = cross(base, reference).normalized();
    let bitangent = cross(base, tangent).normalized();
    let jitter_x = (s.random() * 2.0 - 1.0) * cone;
    let jitter_y = (s.random() * 2.0 - 1.0) * cone;
    base.add(tangent.mul(jitter_x))
        .add(bitangent.mul(jitter_y))
        .normalized()
}
fn cross(a: Vec3, b: Vec3) -> Vec3 {
    Vec3 {
        x: a.y * b.z - a.z * b.y,
        y: a.z * b.x - a.x * b.z,
        z: a.x * b.y - a.y * b.x,
    }
}
impl From<GVec3> for Vec3 {
    fn from(v: GVec3) -> Self {
        Self {
            x: v.x,
            y: v.y,
            z: v.z,
        }
    }
}
