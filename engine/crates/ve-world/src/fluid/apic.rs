//! An APIC liquid (Jiang et al. 2015): water as particles that carry their own velocity and a local affine velocity field, moved by a staggered grid
//! that keeps the flow incompressible. Where the position-based liquid (`Fluid`) squeezes particles apart until their density is right, this one
//! solves a pressure equation every step, so it keeps its volume, sloshes and splashes like water, and does not behave like a bouncy gel.
//!
//! Per step: particles to grid (quadratic B-splines, so the affine terms are exact), gravity and viscosity, solid faces pinned, a pressure projection
//! (red-black SOR, fixed sweeps), velocities spread a few cells into the air, grid back to particles, then the particles move and stay out of
//! the walls and colliders. A small push on over-packed cells stops the volume from drifting. Arithmetic only, in a fixed order, so the same
//! input gives the same bits on every machine and in wasm.

use crate::cloth::{Collider, V3};

use super::{Liquid, Settings};

mod grid;
use grid::Grid;

/// Most particles an APIC liquid may hold (it is stepped every frame).
pub const MAX_APIC_PARTICLES: usize = 60_000;
/// Most grid cells the solver's domain may have.
const MAX_CELLS: usize = 600_000;

/// The liquid: particles `spacing` apart at rest, a grid twice as coarse.
#[derive(Clone)]
pub struct Apic {
    /// Particle positions.
    pub pos: Vec<V3>,
    /// Particle velocities.
    pub vel: Vec<V3>,
    /// Surfaces the liquid cannot enter.
    pub colliders: Vec<Collider>,
    /// How it runs (shared with the position-based liquid: `iterations` scales the pressure sweeps, `substeps` is the most the step may split into).
    pub settings: Settings,
    /// Row-major affine velocity matrix per particle: `c[3 * a + b]` is how velocity component `a` changes along axis `b`.
    affine: Vec<[f32; 9]>,
    spacing: f32,
    origin: V3,
    dims: [usize; 3],
    h: f32,
    previous_colliders: Vec<Collider>,
    pressure: Vec<f32>,
    divergence: (f32, f32),
}

impl std::fmt::Debug for Apic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Apic")
            .field("particles", &self.pos.len())
            .field("dims", &self.dims)
            .field("cell", &self.h)
            .finish()
    }
}

/// A deterministic number in [-1, 1) from an index and a salt (positions are jittered so a lattice does not leave its pattern in the flow).
fn hash(i: usize, salt: u32) -> f32 {
    let mut x = (i as u32)
        .wrapping_mul(0x9E37_79B1)
        .wrapping_add(salt.wrapping_mul(0x85EB_CA6B));
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^= x >> 15;
    (x as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// Quadratic B-spline weights for grid coordinate `g`: the first of three nodes and their weights.
fn spline(g: f32) -> (isize, [f32; 3]) {
    let base = (g - 0.5).floor();
    let f = g - base;
    let w = [
        0.5 * (1.5 - f) * (1.5 - f),
        0.75 - (f - 1.0) * (f - 1.0),
        0.5 * (f - 0.5) * (f - 0.5),
    ];
    (base as isize, w)
}

impl Apic {
    /// A box of liquid filled with particles `spacing` apart, in a domain that is the container's bounds when they are tight, or a window round the
    /// liquid when they are huge. `None` for no particles, past the caps, or numbers that are not finite.
    #[must_use]
    pub fn block(min: V3, max: V3, spacing: f32, bounds: Option<(V3, V3)>) -> Option<Self> {
        if !(spacing.is_finite() && spacing > 1.0e-3)
            || !min.iter().chain(&max).all(|v| v.is_finite())
            || (0..3).any(|a| !(max[a] > min[a]))
        {
            return None;
        }
        let counts: Vec<usize> = (0..3)
            .map(|a| (((max[a] - min[a]) / spacing).floor() as usize).max(1))
            .collect();
        let total = counts[0]
            .saturating_mul(counts[1])
            .saturating_mul(counts[2]);
        if total == 0 || total > MAX_APIC_PARTICLES {
            return None;
        }
        let h = spacing * 2.0;
        // The domain: the block padded by its own size sideways and a half upward, cut by the container, aligned to the cell grid.
        let size = [0, 1, 2].map(|a| max[a] - min[a]);
        let (floor_lo, ceil_hi) = bounds.unwrap_or(([f32::MIN; 3], [f32::MAX; 3]));
        let mut pad = 1.0_f32;
        let mut found = None;
        // A container of a sensible size is the domain itself (the low corner is moved up to the block's lattice, at most one cell in).
        if let Some((lo, hi)) =
            bounds.filter(|(lo, hi)| (0..3).all(|a| hi[a] - lo[a] < 1.0e4 && hi[a] > lo[a]))
        {
            let origin = [0, 1, 2].map(|a| min[a] - ((min[a] - lo[a]) / h).floor().max(0.0) * h);
            let dims = [0, 1, 2].map(|a| (((hi[a] - origin[a]) / h).floor() as usize).max(2));
            let cells = dims[0].saturating_mul(dims[1]).saturating_mul(dims[2]);
            let holds_block = (0..3).all(|a| {
                origin[a] <= min[a] + 1.0e-6 && origin[a] + dims[a] as f32 * h >= max[a] - 1.0e-6
            });
            if cells <= MAX_CELLS && holds_block {
                found = Some((origin, dims));
            }
        }
        for _ in 0..12 {
            if found.is_some() {
                break;
            }
            let lo = [
                (min[0] - size[0] * pad).max(floor_lo[0]),
                // the floor is the container's, unless it is far below the liquid
                if min[1] - floor_lo[1] <= size[1] * 2.0 + 8.0 * h {
                    floor_lo[1]
                } else {
                    min[1] - size[1] * 0.5 * pad
                },
                (min[2] - size[2] * pad).max(floor_lo[2]),
            ];
            let hi = [
                (max[0] + size[0] * pad).min(ceil_hi[0]),
                (max[1] + size[1] * (0.5 + pad)).min(ceil_hi[1]),
                (max[2] + size[2] * pad).min(ceil_hi[2]),
            ];
            // align the low corner to the block's lattice so each cell holds eight particles exactly
            let origin = [0, 1, 2].map(|a| min[a] - ((min[a] - lo[a]) / h).ceil() * h);
            let dims = [0, 1, 2].map(|a| (((hi[a] - origin[a]) / h).ceil() as usize).max(2));
            let cells = dims[0].saturating_mul(dims[1]).saturating_mul(dims[2]);
            if cells <= MAX_CELLS {
                found = Some((origin, dims));
                break;
            }
            pad *= 0.7;
        }
        let (origin, dims) = found?;
        let mut pos = Vec::with_capacity(total);
        let mut index = 0_usize;
        for k in 0..counts[2] {
            for j in 0..counts[1] {
                for i in 0..counts[0] {
                    let jitter = |salt: u32| hash(index, salt) * spacing * 0.12;
                    pos.push([
                        min[0] + (i as f32 + 0.5) * spacing + jitter(1),
                        min[1] + (j as f32 + 0.5) * spacing + jitter(2),
                        min[2] + (k as f32 + 0.5) * spacing + jitter(3),
                    ]);
                    index += 1;
                }
            }
        }
        let n = pos.len();
        Some(Self {
            pos,
            vel: vec![[0.0; 3]; n],
            colliders: Vec::new(),
            settings: Settings {
                bounds,
                ..Settings::default()
            },
            affine: vec![[0.0; 9]; n],
            spacing,
            origin,
            dims,
            h,
            previous_colliders: Vec::new(),
            pressure: vec![0.0; dims[0] * dims[1] * dims[2]],
            divergence: (0.0, 0.0),
        })
    }

    /// Lattice spacing of the particles at rest, in metres.
    #[must_use]
    pub fn spacing(&self) -> f32 {
        self.spacing
    }

    /// Size of one grid cell, in metres.
    #[must_use]
    pub fn cell_size(&self) -> f32 {
        self.h
    }

    /// The grid's cells along each axis and its low corner.
    #[must_use]
    pub fn domain(&self) -> ([usize; 3], V3) {
        (self.dims, self.origin)
    }

    /// Kinetic energy of the particles per unit mass, in joules per kilogram of liquid (half the sum of speeds squared over the count).
    #[must_use]
    pub fn kinetic_energy(&self) -> f32 {
        let sum: f32 = self
            .vel
            .iter()
            .map(|v| v[0] * v[0] + v[1] * v[1] + v[2] * v[2])
            .sum();
        0.5 * sum / self.pos.len().max(1) as f32
    }

    /// The worst divergence (1/s) in any liquid cell just before and just after the last pressure projection: the second is what the solver left,
    /// the first what it had to remove.
    #[must_use]
    pub fn last_divergence(&self) -> (f32, f32) {
        self.divergence
    }

    fn fresh_grid(&self) -> Grid {
        let mut grid = Grid::new(self.origin, self.dims, self.h);
        grid.pressure.clone_from(&self.pressure);
        // solids: collider interiors (the domain walls are the faces on the edge, which `is_solid` reports for anything outside)
        if !self.colliders.is_empty() {
            for k in 0..self.dims[2] {
                for j in 0..self.dims[1] {
                    for i in 0..self.dims[0] {
                        let centre = grid.centre(i, j, k);
                        for (n, collider) in self.colliders.iter().enumerate() {
                            if collider.distance(centre).0 < 0.3 * self.h {
                                let c = grid.cell(i, j, k);
                                grid.solid[c] = 2 + n.min(250) as u8;
                                break;
                            }
                        }
                    }
                }
            }
        }
        for p in &self.pos {
            let c = [0, 1, 2].map(|a| ((p[a] - self.origin[a]) / self.h).floor() as isize);
            if (0..3).all(|a| c[a] >= 0 && c[a] < self.dims[a] as isize) {
                let idx = grid.cell(c[0] as usize, c[1] as usize, c[2] as usize);
                grid.count[idx] += 1;
            }
        }
        grid
    }

    /// Splats every particle's momentum (velocity plus the affine term) onto the faces with quadratic weights, and divides by the weight sums.
    fn particles_to_grid(&self, grid: &mut Grid) {
        for a in 0..3 {
            grid.u[a].iter_mut().for_each(|v| *v = 0.0);
            grid.w[a].iter_mut().for_each(|v| *v = 0.0);
        }
        let inv_h = 1.0 / self.h;
        for (n, p) in self.pos.iter().enumerate() {
            for a in 0..3 {
                // velocity component `a` lives on faces: on the face's own axis the nodes are at integer coordinates, across it at half integers
                let g = [0, 1, 2]
                    .map(|b| (p[b] - self.origin[b]) * inv_h - if b == a { 0.0 } else { 0.5 });
                let (bx, wx) = spline(g[0]);
                let (by, wy) = spline(g[1]);
                let (bz, wz) = spline(g[2]);
                let fd = grid.face_dims(a);
                let c = &self.affine[n];
                for (dk, wk) in wz.iter().enumerate() {
                    let k = bz + dk as isize;
                    if k < 0 || k >= fd[2] as isize {
                        continue;
                    }
                    for (dj, wj) in wy.iter().enumerate() {
                        let j = by + dj as isize;
                        if j < 0 || j >= fd[1] as isize {
                            continue;
                        }
                        for (di, wi) in wx.iter().enumerate() {
                            let i = bx + di as isize;
                            if i < 0 || i >= fd[0] as isize {
                                continue;
                            }
                            let w = wi * wj * wk;
                            // offset from particle to node, in metres
                            let d = [
                                (i as f32 - g[0]) * self.h,
                                (j as f32 - g[1]) * self.h,
                                (k as f32 - g[2]) * self.h,
                            ];
                            let momentum = self.vel[n][a]
                                + c[3 * a] * d[0]
                                + c[3 * a + 1] * d[1]
                                + c[3 * a + 2] * d[2];
                            let f = i as usize + fd[0] * (j as usize + fd[1] * k as usize);
                            grid.u[a][f] += w * momentum;
                            grid.w[a][f] += w;
                        }
                    }
                }
            }
        }
        for a in 0..3 {
            for (u, w) in grid.u[a].iter_mut().zip(&grid.w[a]) {
                *u = if *w > 1.0e-6 { *u / *w } else { 0.0 };
            }
        }
    }

    /// Reads the new velocity and affine matrix of every particle from the faces.
    fn grid_to_particles(&mut self, grid: &Grid) {
        let inv_h = 1.0 / self.h;
        let four_over_h2 = 4.0 * inv_h * inv_h;
        for n in 0..self.pos.len() {
            let p = self.pos[n];
            let mut v = [0.0_f32; 3];
            let mut c = [0.0_f32; 9];
            for a in 0..3 {
                let g = [0, 1, 2]
                    .map(|b| (p[b] - self.origin[b]) * inv_h - if b == a { 0.0 } else { 0.5 });
                let (bx, wx) = spline(g[0]);
                let (by, wy) = spline(g[1]);
                let (bz, wz) = spline(g[2]);
                let fd = grid.face_dims(a);
                for (dk, wk) in wz.iter().enumerate() {
                    let k = (bz + dk as isize).clamp(0, fd[2] as isize - 1);
                    for (dj, wj) in wy.iter().enumerate() {
                        let j = (by + dj as isize).clamp(0, fd[1] as isize - 1);
                        for (di, wi) in wx.iter().enumerate() {
                            let i = (bx + di as isize).clamp(0, fd[0] as isize - 1);
                            let w = wi * wj * wk;
                            let u =
                                grid.u[a][i as usize + fd[0] * (j as usize + fd[1] * k as usize)];
                            let d = [
                                ((bx + di as isize) as f32 - g[0]) * self.h,
                                ((by + dj as isize) as f32 - g[1]) * self.h,
                                ((bz + dk as isize) as f32 - g[2]) * self.h,
                            ];
                            v[a] += w * u;
                            for b in 0..3 {
                                c[3 * a + b] += four_over_h2 * w * u * d[b];
                            }
                        }
                    }
                }
            }
            self.vel[n] = v;
            self.affine[n] = c;
        }
    }

    /// Advances the liquid by `dt` seconds (split into steps that keep the fastest particle under a cell's width); false (and the liquid reset to
    /// nothing moving) if a position went non-finite.
    pub fn step(&mut self, dt: f32) -> bool {
        if !(dt.is_finite() && dt > 0.0) {
            return true;
        }
        let fastest = self
            .vel
            .iter()
            .map(|v| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt())
            .fold(0.0_f32, f32::max);
        // fall speed over one frame counts too, so a liquid that starts at rest is not taken in one huge step
        let reach = fastest
            + self
                .settings
                .gravity
                .iter()
                .map(|g| g * g)
                .sum::<f32>()
                .sqrt()
                * dt;
        let most = (self.settings.substeps.max(1) * 3).clamp(1, 12);
        let steps = (((reach * dt) / (0.8 * self.h)).ceil() as u32).clamp(1, most);
        let sub = dt / steps as f32;
        for _ in 0..steps {
            self.substep(sub);
        }
        self.previous_colliders = self.colliders.clone();
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

    fn substep(&mut self, dt: f32) {
        let solid_velocity = self.collider_velocities(dt);
        let mut grid = self.fresh_grid();
        self.particles_to_grid(&mut grid);
        grid.mark_valid();
        // gravity on every face next to liquid
        for a in 0..3 {
            let g = self.settings.gravity[a];
            if g == 0.0 {
                continue;
            }
            for (u, valid) in grid.u[a].iter_mut().zip(&grid.valid[a]) {
                if *valid {
                    *u += g * dt;
                }
            }
        }
        grid.diffuse((self.settings.viscosity * 0.5).clamp(0.0, 0.2));
        grid.enforce_solids(&solid_velocity);
        // over-packed cells are allowed to push outward a little
        let rest = (self.h / self.spacing).powi(3);
        let mut target = vec![0.0_f32; grid.cells()];
        for (t, count) in target.iter_mut().zip(&grid.count) {
            let density = *count as f32 / rest;
            if density > 1.0 {
                *t = (0.05 / dt) * (density - 1.0).min(1.0);
            }
        }
        let sweeps = (self.settings.iterations.max(1) * 16).min(128) as usize;
        let before = grid.worst_divergence(&target);
        grid.project(dt, sweeps, &target);
        grid.enforce_solids(&solid_velocity);
        self.divergence = (before, grid.worst_divergence(&target));
        grid.mark_valid();
        grid.extrapolate(3);
        grid.enforce_solids(&solid_velocity);
        self.pressure.clone_from(&grid.pressure);
        self.grid_to_particles(&grid);
        // cap runaway speeds at what the step can carry
        let limit = 1.5 * self.h / dt;
        for v in &mut self.vel {
            let s = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            if s > limit {
                for x in v.iter_mut() {
                    *x *= limit / s;
                }
            }
        }
        for n in 0..self.pos.len() {
            for a in 0..3 {
                self.pos[n][a] += self.vel[n][a] * dt;
            }
            self.collide(n);
        }
    }

    /// Velocity of each collider over the last step, from where it was.
    fn collider_velocities(&self, dt: f32) -> Vec<V3> {
        self.colliders
            .iter()
            .enumerate()
            .map(|(n, now)| match (now, self.previous_colliders.get(n)) {
                (Collider::Capsule { a, b, .. }, Some(Collider::Capsule { a: pa, b: pb, .. })) => {
                    [0, 1, 2].map(|k| ((a[k] + b[k]) - (pa[k] + pb[k])) * 0.5 / dt)
                }
                (
                    Collider::Sphere { center, .. },
                    Some(Collider::Sphere { center: before, .. }),
                ) => [0, 1, 2].map(|k| (center[k] - before[k]) / dt),
                _ => [0.0; 3],
            })
            .collect()
    }

    /// Keeps particle `n` out of the colliders and inside the walls, removing the velocity into them.
    fn collide(&mut self, n: usize) {
        let radius = self.spacing * 0.5;
        let friction = self.settings.friction;
        let mut p = self.pos[n];
        let mut v = self.vel[n];
        let mut touched = false;
        for collider in &self.colliders {
            let (d, normal) = collider.distance(p);
            if d < radius {
                for a in 0..3 {
                    p[a] += normal[a] * (radius - d);
                }
                let into = v[0] * normal[0] + v[1] * normal[1] + v[2] * normal[2];
                if into < 0.0 {
                    for a in 0..3 {
                        v[a] = (v[a] - normal[a] * into) * (1.0 - friction);
                    }
                }
                touched = true;
            }
        }
        // the domain walls are one cell in from the edge, where the solid faces are
        let (lo, hi) = (
            [0, 1, 2].map(|a| self.origin[a] + radius),
            [0, 1, 2].map(|a| self.origin[a] + self.dims[a] as f32 * self.h - radius),
        );
        for a in 0..3 {
            if p[a] < lo[a] {
                p[a] = lo[a];
                if v[a] < 0.0 {
                    v[a] = 0.0;
                }
                touched = true;
            } else if p[a] > hi[a] {
                p[a] = hi[a];
                if v[a] > 0.0 {
                    v[a] = 0.0;
                }
                touched = true;
            }
        }
        if touched {
            // a particle that hit something forgets its local swirl, which would otherwise ring
            self.affine[n] = [0.0; 9];
        }
        self.pos[n] = p;
        self.vel[n] = v;
    }
}

impl Liquid for Apic {
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
        Apic::step(self, dt)
    }
}
