//! Position-based fluids (Macklin and Mueller 2013): water and other liquids as particles that keep a constant density by moving each other apart.
//! The same family as the cloth solver (`cloth`), so it shares its colliders (sphere, capsule, plane, signed-distance grid) and its guarantees:
//! arithmetic and `sqrt` only, a fixed order for every sum, so the same input gives the same bits on every machine and in wasm.
//!
//! Per substep: gravity, predict, find neighbours (a sorted grid), a few density-constraint passes, collisions, then velocities from the
//! moved positions and an XSPH viscosity blend. `block` fills a box with particles on a lattice.

use crate::cloth::{Collider, V3};

mod apic;
mod surface;
pub use apic::{Apic, MAX_APIC_PARTICLES};
pub use surface::{MAX_GRID, Surface, surface};

/// What the runtime needs of a liquid, whichever solver runs it: where the particles are, how fast they move, the colliders to follow the bones, and a step.
pub trait Liquid: Send {
    /// Replaces how the liquid runs (gravity, viscosity, container and so on).
    fn configure(&mut self, settings: Settings);
    /// Particle positions.
    fn pos(&self) -> &[V3];
    /// Particle velocities.
    fn vel(&self) -> &[V3];
    /// The surfaces the liquid cannot enter, replaced each frame.
    fn colliders_mut(&mut self) -> &mut Vec<Collider>;
    /// Advances by `dt` seconds; false (and the liquid at rest) if a position went non-finite.
    fn step(&mut self, dt: f32) -> bool;
}

impl Liquid for Fluid {
    fn configure(&mut self, settings: Settings) {
        self.settings = settings;
    }
    fn pos(&self) -> &[V3] {
        &self.pos
    }
    fn vel(&self) -> &[V3] {
        &self.vel
    }
    fn colliders_mut(&mut self) -> &mut Vec<Collider> {
        &mut self.colliders
    }
    fn step(&mut self, dt: f32) -> bool {
        Fluid::step(self, dt)
    }
}

/// Most particles one fluid may hold (it is stepped every frame).
pub const MAX_PARTICLES: usize = 20_000;

/// How a fluid runs.
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// Acceleration in metres per second squared.
    pub gravity: V3,
    /// Steps per frame.
    pub substeps: u32,
    /// Density passes per step.
    pub iterations: u32,
    /// XSPH velocity blending with neighbours, 0 to 1 (honey is high, water low).
    pub viscosity: f32,
    /// Constraint softening; larger is squishier and steadier.
    pub relaxation: f32,
    /// Share of sliding speed lost on contact with a collider or the walls.
    pub friction: f32,
    /// The walls of the container (min, max corners); particles stay inside.
    pub bounds: Option<(V3, V3)>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            gravity: [0.0, -9.81, 0.0],
            substeps: 3,
            iterations: 4,
            viscosity: 0.02,
            relaxation: 600.0,
            friction: 0.1,
            bounds: None,
        }
    }
}

/// The liquid: particles of equal mass at `spacing` apart at rest.
#[derive(Clone, Debug)]
pub struct Fluid {
    /// Particle positions.
    pub pos: Vec<V3>,
    /// Particle velocities.
    pub vel: Vec<V3>,
    /// Surfaces the liquid cannot enter.
    pub colliders: Vec<Collider>,
    /// How it runs.
    pub settings: Settings,
    spacing: f32,
    radius: f32,
    rest_density: f32,
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Smoothing kernels with support `h`.
struct Kernel {
    h: f32,
    poly6: f32,
    spiky: f32,
}

impl Kernel {
    fn new(h: f32) -> Self {
        let pi = std::f32::consts::PI;
        Self {
            h,
            poly6: 315.0 / (64.0 * pi * h.powi(9)),
            spiky: -45.0 / (pi * h.powi(6)),
        }
    }
    fn w(&self, r2: f32) -> f32 {
        let d = self.h * self.h - r2;
        if d <= 0.0 {
            0.0
        } else {
            self.poly6 * d * d * d
        }
    }
    /// Gradient of the spiky kernel at offset `d` (length `r`), pointing from the neighbour to the particle's growing direction.
    fn grad(&self, d: V3, r: f32) -> V3 {
        if r <= 1.0e-9 || r >= self.h {
            return [0.0; 3];
        }
        let s = self.spiky * (self.h - r) * (self.h - r) / r;
        [d[0] * s, d[1] * s, d[2] * s]
    }
}

impl Fluid {
    /// A box of liquid filled with particles `spacing` apart; `None` for no particles or past the cap.
    #[must_use]
    pub fn block(min: V3, max: V3, spacing: f32) -> Option<Self> {
        if !(spacing.is_finite() && spacing > 1.0e-4) || (0..3).any(|a| !(max[a] > min[a])) {
            return None;
        }
        let counts: Vec<usize> = (0..3)
            .map(|a| (((max[a] - min[a]) / spacing).floor() as usize).max(1))
            .collect();
        let total = counts[0]
            .saturating_mul(counts[1])
            .saturating_mul(counts[2]);
        if total == 0 || total > MAX_PARTICLES {
            return None;
        }
        let mut pos = Vec::with_capacity(total);
        for k in 0..counts[2] {
            for j in 0..counts[1] {
                for i in 0..counts[0] {
                    pos.push([
                        min[0] + (i as f32 + 0.5) * spacing,
                        min[1] + (j as f32 + 0.5) * spacing,
                        min[2] + (k as f32 + 0.5) * spacing,
                    ]);
                }
            }
        }
        let radius = spacing * 2.0;
        // Rest density: the kernel sum at one lattice point (a full neighbourhood), so a settled liquid reads one.
        let kernel = Kernel::new(radius);
        let reach = 2;
        let mut rest = 0.0;
        for k in -reach..=reach {
            for j in -reach..=reach {
                for i in -reach..=reach {
                    let d = [i as f32 * spacing, j as f32 * spacing, k as f32 * spacing];
                    rest += kernel.w(dot(d, d));
                }
            }
        }
        let vel = vec![[0.0; 3]; pos.len()];
        Some(Self {
            pos,
            vel,
            colliders: Vec::new(),
            settings: Settings::default(),
            spacing,
            radius,
            rest_density: rest,
        })
    }

    /// Lattice spacing of the particles at rest, in metres.
    #[must_use]
    pub fn spacing(&self) -> f32 {
        self.spacing
    }

    /// The density of particle `i` relative to rest (1 is settled liquid; a free surface reads lower).
    #[must_use]
    pub fn density_ratio(&self, i: usize) -> f32 {
        let kernel = Kernel::new(self.radius);
        let grid = Grid::build(&self.pos, self.radius);
        let mut rho = 0.0;
        grid.each_neighbour(&self.pos, i, |j| {
            let d = sub(self.pos[i], self.pos[j]);
            rho += kernel.w(dot(d, d));
        });
        rho / self.rest_density
    }

    /// Advances the liquid by `dt` seconds; false (and the liquid reset to nothing moving) if a position went non-finite.
    pub fn step(&mut self, dt: f32) -> bool {
        let n = self.pos.len();
        let sub_dt = dt / self.settings.substeps.max(1) as f32;
        let kernel = Kernel::new(self.radius);
        let g = self.settings.gravity;
        let particle_radius = self.spacing * 0.5;
        for _ in 0..self.settings.substeps.max(1) {
            let old = self.pos.clone();
            for i in 0..n {
                for a in 0..3 {
                    self.vel[i][a] += g[a] * sub_dt;
                    self.pos[i][a] += self.vel[i][a] * sub_dt;
                }
                self.collide(i, particle_radius);
            }
            let grid = Grid::build(&self.pos, self.radius);
            let mut lambda = vec![0.0_f32; n];
            for _ in 0..self.settings.iterations.max(1) {
                for i in 0..n {
                    let (mut rho, mut grad_i, mut sum) = (0.0_f32, [0.0_f32; 3], 0.0_f32);
                    grid.each_neighbour(&self.pos, i, |j| {
                        let d = sub(self.pos[i], self.pos[j]);
                        let r2 = dot(d, d);
                        rho += kernel.w(r2);
                        if j != i {
                            let gr = kernel.grad(d, r2.sqrt());
                            for a in 0..3 {
                                grad_i[a] += gr[a];
                            }
                            let gj = [
                                gr[0] / self.rest_density,
                                gr[1] / self.rest_density,
                                gr[2] / self.rest_density,
                            ];
                            sum += dot(gj, gj);
                        }
                    });
                    let gi = [
                        grad_i[0] / self.rest_density,
                        grad_i[1] / self.rest_density,
                        grad_i[2] / self.rest_density,
                    ];
                    sum += dot(gi, gi);
                    // Only compress: a free surface reads under rest density and must not pull itself together.
                    let c = (rho / self.rest_density - 1.0).max(0.0);
                    lambda[i] = -c / (sum + self.settings.relaxation);
                }
                let mut delta = vec![[0.0_f32; 3]; n];
                for i in 0..n {
                    let mut d_i = [0.0_f32; 3];
                    grid.each_neighbour(&self.pos, i, |j| {
                        if j == i {
                            return;
                        }
                        let d = sub(self.pos[i], self.pos[j]);
                        let r = dot(d, d).sqrt();
                        // Artificial pressure keeps particles from clumping at a free surface.
                        let q = kernel.w(r * r) / kernel.w((0.2 * self.radius).powi(2));
                        let corr = -1.0e-4 * q * q * q * q;
                        let gr = kernel.grad(d, r);
                        let s = (lambda[i] + lambda[j] + corr) / self.rest_density;
                        for a in 0..3 {
                            d_i[a] += gr[a] * s;
                        }
                    });
                    delta[i] = d_i;
                }
                for i in 0..n {
                    for a in 0..3 {
                        self.pos[i][a] += delta[i][a];
                    }
                    self.collide(i, particle_radius);
                }
            }
            for i in 0..n {
                for a in 0..3 {
                    self.vel[i][a] = (self.pos[i][a] - old[i][a]) / sub_dt;
                }
            }
            if self.settings.viscosity > 0.0 {
                let grid = Grid::build(&self.pos, self.radius);
                let mut blended = self.vel.clone();
                for i in 0..n {
                    let mut dv = [0.0_f32; 3];
                    grid.each_neighbour(&self.pos, i, |j| {
                        if j == i {
                            return;
                        }
                        let d = sub(self.pos[i], self.pos[j]);
                        let w = kernel.w(dot(d, d)) / self.rest_density;
                        for a in 0..3 {
                            dv[a] += (self.vel[j][a] - self.vel[i][a]) * w;
                        }
                    });
                    for a in 0..3 {
                        blended[i][a] += self.settings.viscosity * dv[a];
                    }
                }
                self.vel = blended;
            }
        }
        if self
            .pos
            .iter()
            .chain(&self.vel)
            .flatten()
            .all(|v| v.is_finite())
        {
            true
        } else {
            for v in &mut self.vel {
                *v = [0.0; 3];
            }
            false
        }
    }

    /// Keeps particle `i` out of the colliders and inside the walls, and takes friction from what it touches.
    fn collide(&mut self, i: usize, radius: f32) {
        let friction = self.settings.friction;
        let touch = |p: &mut V3, normal: V3, depth: f32, v: &mut V3| {
            for a in 0..3 {
                p[a] += normal[a] * depth;
            }
            let into = dot(*v, normal);
            if into < 0.0 {
                let slide = [
                    v[0] - normal[0] * into,
                    v[1] - normal[1] * into,
                    v[2] - normal[2] * into,
                ];
                for a in 0..3 {
                    v[a] = slide[a] * (1.0 - friction);
                }
            }
        };
        let mut p = self.pos[i];
        let mut v = self.vel[i];
        for collider in &self.colliders {
            let (d, normal) = collider.distance(p);
            if d < radius {
                touch(&mut p, normal, radius - d, &mut v);
            }
        }
        if let Some((lo, hi)) = self.settings.bounds {
            for a in 0..3 {
                let mut n = [0.0; 3];
                if p[a] < lo[a] + radius {
                    n[a] = 1.0;
                    let depth = lo[a] + radius - p[a];
                    touch(&mut p, n, depth, &mut v);
                } else if p[a] > hi[a] - radius {
                    n[a] = -1.0;
                    let depth = p[a] - (hi[a] - radius);
                    touch(&mut p, n, depth, &mut v);
                }
            }
        }
        self.pos[i] = p;
        self.vel[i] = v;
    }
}

/// A uniform grid of cell size `h` over the particles: the (cell, index) pairs sorted, so a query walks 27 cells in a fixed order.
struct Grid {
    h: f32,
    keys: Vec<(i64, u32)>,
}

fn cell_of(p: V3, h: f32) -> [i64; 3] {
    [
        (p[0] / h).floor() as i64,
        (p[1] / h).floor() as i64,
        (p[2] / h).floor() as i64,
    ]
}

fn key(c: [i64; 3]) -> i64 {
    // 21 bits per axis, enough for a 2-million-cell range either way of the origin.
    let m = (1_i64 << 20) - 1;
    ((c[0] & m) << 42) | ((c[1] & m) << 21) | (c[2] & m)
}

impl Grid {
    fn build(pos: &[V3], h: f32) -> Self {
        let mut keys: Vec<(i64, u32)> = pos
            .iter()
            .enumerate()
            .map(|(i, p)| (key(cell_of(*p, h)), i as u32))
            .collect();
        keys.sort_unstable();
        Self { h, keys }
    }

    fn each_neighbour(&self, pos: &[V3], i: usize, mut f: impl FnMut(usize)) {
        let c = cell_of(pos[i], self.h);
        let h2 = self.h * self.h;
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let k = key([c[0] + dx, c[1] + dy, c[2] + dz]);
                    let start = self.keys.partition_point(|e| e.0 < k);
                    for e in &self.keys[start..] {
                        if e.0 != k {
                            break;
                        }
                        let j = e.1 as usize;
                        let d = sub(pos[i], pos[j]);
                        if dot(d, d) < h2 {
                            f(j);
                        }
                    }
                }
            }
        }
    }
}
