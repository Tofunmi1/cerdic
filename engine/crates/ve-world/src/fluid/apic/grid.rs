//! The staggered (MAC) grid of the APIC liquid: velocities on cell faces, a solid mask, and the pressure projection that makes the flow incompressible.
//!
//! Face velocity `u[a]` lives on the faces of cells that are normal to axis `a`; it has one more entry along `a` than there are cells. A cell holds
//! liquid when a particle is in it, is solid when it is outside the domain or inside a collider, and is air otherwise (pressure zero). Every sum and
//! sweep runs in a fixed order with plain arithmetic, so the same input gives the same bits everywhere.

use crate::cloth::V3;

/// Neighbour offsets of the six faces of a cell: (axis, side) with side 0 the low face and 1 the high face.
pub(super) const FACES: [(usize, usize); 6] = [(0, 0), (0, 1), (1, 0), (1, 1), (2, 0), (2, 1)];

/// The solid mask value of the domain walls and any static solid.
pub(super) const WALL: u8 = 1;

pub(super) struct Grid {
    pub dims: [usize; 3],
    pub h: f32,
    pub origin: V3,
    /// Face velocities by axis.
    pub u: [Vec<f32>; 3],
    /// Weight sums of the transfer to each face (zero where no particle reached).
    pub w: [Vec<f32>; 3],
    /// 0 free, `WALL` for the domain walls, `2 + n` for collider `n`.
    pub solid: Vec<u8>,
    /// Particles in each cell.
    pub count: Vec<u32>,
    pub pressure: Vec<f32>,
    /// Which faces carry a trustworthy velocity (next to liquid, or filled by extrapolation).
    pub valid: [Vec<bool>; 3],
}

impl Grid {
    pub(super) fn new(origin: V3, dims: [usize; 3], h: f32) -> Self {
        let cells = dims[0] * dims[1] * dims[2];
        let face_len = |a: usize| {
            let mut d = dims;
            d[a] += 1;
            d[0] * d[1] * d[2]
        };
        Self {
            dims,
            h,
            origin,
            u: [
                vec![0.0; face_len(0)],
                vec![0.0; face_len(1)],
                vec![0.0; face_len(2)],
            ],
            w: [
                vec![0.0; face_len(0)],
                vec![0.0; face_len(1)],
                vec![0.0; face_len(2)],
            ],
            solid: vec![0; cells],
            count: vec![0; cells],
            pressure: vec![0.0; cells],
            valid: [
                vec![false; face_len(0)],
                vec![false; face_len(1)],
                vec![false; face_len(2)],
            ],
        }
    }

    pub(super) fn cells(&self) -> usize {
        self.dims[0] * self.dims[1] * self.dims[2]
    }

    pub(super) fn cell(&self, i: usize, j: usize, k: usize) -> usize {
        i + self.dims[0] * (j + self.dims[1] * k)
    }

    /// The face array dimensions of axis `a`.
    pub(super) fn face_dims(&self, a: usize) -> [usize; 3] {
        let mut d = self.dims;
        d[a] += 1;
        d
    }

    pub(super) fn face(&self, a: usize, i: usize, j: usize, k: usize) -> usize {
        let d = self.face_dims(a);
        i + d[0] * (j + d[1] * k)
    }

    /// Centre of cell (i, j, k).
    pub(super) fn centre(&self, i: usize, j: usize, k: usize) -> V3 {
        [
            self.origin[0] + (i as f32 + 0.5) * self.h,
            self.origin[1] + (j as f32 + 0.5) * self.h,
            self.origin[2] + (k as f32 + 0.5) * self.h,
        ]
    }

    /// Whether the cell at signed indices is solid; outside the domain counts as wall.
    pub(super) fn is_solid(&self, i: isize, j: isize, k: isize) -> bool {
        if i < 0
            || j < 0
            || k < 0
            || i >= self.dims[0] as isize
            || j >= self.dims[1] as isize
            || k >= self.dims[2] as isize
        {
            return true;
        }
        self.solid[self.cell(i as usize, j as usize, k as usize)] != 0
    }

    fn is_liquid(&self, i: isize, j: isize, k: isize) -> bool {
        if i < 0
            || j < 0
            || k < 0
            || i >= self.dims[0] as isize
            || j >= self.dims[1] as isize
            || k >= self.dims[2] as isize
        {
            return false;
        }
        let c = self.cell(i as usize, j as usize, k as usize);
        self.solid[c] == 0 && self.count[c] > 0
    }

    /// Gives the velocity of the solid at a face a collider owns (zero for the walls). `solid_velocity[n]` is the velocity of collider `n`.
    fn solid_speed(&self, owner: u8, axis: usize, solid_velocity: &[V3]) -> f32 {
        if owner >= 2 {
            solid_velocity
                .get(usize::from(owner) - 2)
                .map_or(0.0, |v| v[axis])
        } else {
            0.0
        }
    }

    /// Pins the velocity of every face next to a solid cell to the solid's own (so liquid neither enters nor leaves it).
    pub(super) fn enforce_solids(&mut self, solid_velocity: &[V3]) {
        for a in 0..3 {
            let fd = self.face_dims(a);
            for k in 0..fd[2] {
                for j in 0..fd[1] {
                    for i in 0..fd[0] {
                        let here = [i as isize, j as isize, k as isize];
                        let mut behind = here;
                        behind[a] -= 1;
                        let (s_here, s_behind) = (self.owner(here), self.owner(behind));
                        if s_here == 0 && s_behind == 0 {
                            continue;
                        }
                        let owner = if s_here != 0 { s_here } else { s_behind };
                        let f = self.face(a, i, j, k);
                        self.u[a][f] = self.solid_speed(owner, a, solid_velocity);
                    }
                }
            }
        }
    }

    /// The solid mask value at signed indices (the wall outside the domain).
    fn owner(&self, c: [isize; 3]) -> u8 {
        if c[0] < 0
            || c[1] < 0
            || c[2] < 0
            || c[0] >= self.dims[0] as isize
            || c[1] >= self.dims[1] as isize
            || c[2] >= self.dims[2] as isize
        {
            return WALL;
        }
        self.solid[self.cell(c[0] as usize, c[1] as usize, c[2] as usize)]
    }

    /// Marks which faces are next to liquid; only those hold a velocity that came from particles.
    pub(super) fn mark_valid(&mut self) {
        for a in 0..3 {
            let fd = self.face_dims(a);
            for k in 0..fd[2] {
                for j in 0..fd[1] {
                    for i in 0..fd[0] {
                        let here = [i as isize, j as isize, k as isize];
                        let mut behind = here;
                        behind[a] -= 1;
                        let touching = self.is_liquid(here[0], here[1], here[2])
                            || self.is_liquid(behind[0], behind[1], behind[2]);
                        let f = self.face(a, i, j, k);
                        self.valid[a][f] = touching;
                    }
                }
            }
        }
    }

    /// Spreads velocities from valid faces into the air around the liquid (a few layers), so a particle near the surface reads something sensible.
    pub(super) fn extrapolate(&mut self, layers: usize) {
        for _ in 0..layers {
            for a in 0..3 {
                let fd = self.face_dims(a);
                let mut next_valid = self.valid[a].clone();
                let mut next_u = self.u[a].clone();
                for k in 0..fd[2] {
                    for j in 0..fd[1] {
                        for i in 0..fd[0] {
                            let f = self.face(a, i, j, k);
                            if self.valid[a][f] {
                                continue;
                            }
                            let (mut sum, mut n) = (0.0_f32, 0.0_f32);
                            for (axis, side) in FACES {
                                let mut c = [i as isize, j as isize, k as isize];
                                c[axis] += if side == 0 { -1 } else { 1 };
                                if c[0] < 0
                                    || c[1] < 0
                                    || c[2] < 0
                                    || c[0] >= fd[0] as isize
                                    || c[1] >= fd[1] as isize
                                    || c[2] >= fd[2] as isize
                                {
                                    continue;
                                }
                                let g = self.face(a, c[0] as usize, c[1] as usize, c[2] as usize);
                                if self.valid[a][g] {
                                    sum += self.u[a][g];
                                    n += 1.0;
                                }
                            }
                            if n > 0.0 {
                                next_u[f] = sum / n;
                                next_valid[f] = true;
                            }
                        }
                    }
                }
                self.u[a] = next_u;
                self.valid[a] = next_valid;
            }
        }
    }

    /// Smooths the face velocities a little where there is liquid (viscosity: honey is a high `amount`). Solid faces keep their value.
    pub(super) fn diffuse(&mut self, amount: f32) {
        if amount <= 0.0 {
            return;
        }
        for a in 0..3 {
            let fd = self.face_dims(a);
            let old = self.u[a].clone();
            for k in 0..fd[2] {
                for j in 0..fd[1] {
                    for i in 0..fd[0] {
                        let f = self.face(a, i, j, k);
                        let here = [i as isize, j as isize, k as isize];
                        let mut behind = here;
                        behind[a] -= 1;
                        if self.owner(here) != 0 || self.owner(behind) != 0 || !self.valid[a][f] {
                            continue;
                        }
                        let (mut sum, mut n) = (0.0_f32, 0.0_f32);
                        for (axis, side) in FACES {
                            let mut c = [i as isize, j as isize, k as isize];
                            c[axis] += if side == 0 { -1 } else { 1 };
                            if c[0] < 0
                                || c[1] < 0
                                || c[2] < 0
                                || c[0] >= fd[0] as isize
                                || c[1] >= fd[1] as isize
                                || c[2] >= fd[2] as isize
                            {
                                continue;
                            }
                            let g = self.face(a, c[0] as usize, c[1] as usize, c[2] as usize);
                            if self.valid[a][g] {
                                sum += old[g];
                                n += 1.0;
                            }
                        }
                        if n > 0.0 {
                            self.u[a][f] = old[f] * (1.0 - amount) + (sum / n) * amount;
                        }
                    }
                }
            }
        }
    }

    /// The divergence of the face velocities in a cell, per second.
    pub(super) fn divergence(&self, i: usize, j: usize, k: usize) -> f32 {
        let mut d = 0.0;
        for a in 0..3 {
            let lo = self.u[a][self.face(a, i, j, k)];
            let hi = self.u[a][match a {
                0 => self.face(0, i + 1, j, k),
                1 => self.face(1, i, j + 1, k),
                _ => self.face(2, i, j, k + 1),
            }];
            d += hi - lo;
        }
        d / self.h
    }

    /// The worst divergence in any liquid cell beyond what `target` allows, per second.
    pub(super) fn worst_divergence(&self, target: &[f32]) -> f32 {
        let mut worst = 0.0_f32;
        for k in 0..self.dims[2] {
            for j in 0..self.dims[1] {
                for i in 0..self.dims[0] {
                    if self.is_liquid(i as isize, j as isize, k as isize) {
                        worst = worst
                            .max((self.divergence(i, j, k) - target[self.cell(i, j, k)]).abs());
                    }
                }
            }
        }
        worst
    }

    /// Makes the flow incompressible: solves for the pressure that removes the divergence of the liquid cells (red-black SOR with a fixed number
    /// of sweeps, warm-started from the last step) and subtracts its gradient from the face velocities. `target` is a per-cell divergence the
    /// flow is allowed to have (a small outward push in over-packed cells, which keeps the volume from drifting).
    pub(super) fn project(&mut self, dt: f32, sweeps: usize, target: &[f32]) {
        let h = self.h;
        let omega = 1.7_f32;
        let mut rhs = vec![0.0_f32; self.cells()];
        let mut diag = vec![0.0_f32; self.cells()];
        for k in 0..self.dims[2] {
            for j in 0..self.dims[1] {
                for i in 0..self.dims[0] {
                    let c = self.cell(i, j, k);
                    if !self.is_liquid(i as isize, j as isize, k as isize) {
                        self.pressure[c] = 0.0;
                        continue;
                    }
                    let mut open = 0.0;
                    for (axis, side) in FACES {
                        let mut n = [i as isize, j as isize, k as isize];
                        n[axis] += if side == 0 { -1 } else { 1 };
                        if !self.is_solid(n[0], n[1], n[2]) {
                            open += 1.0;
                        }
                    }
                    diag[c] = open;
                    // div' = div + dt/h^2 * sum(p - p_neighbour) must equal the target, so the sum is h^2/dt times (target - div)
                    rhs[c] = (h * h / dt) * (target[c] - self.divergence(i, j, k));
                }
            }
        }
        for _ in 0..sweeps {
            for colour in 0..2 {
                for k in 0..self.dims[2] {
                    for j in 0..self.dims[1] {
                        for i in 0..self.dims[0] {
                            if (i + j + k + colour) % 2 != 0 {
                                continue;
                            }
                            let c = self.cell(i, j, k);
                            if diag[c] == 0.0 {
                                continue;
                            }
                            let mut around = 0.0;
                            for (axis, side) in FACES {
                                let mut n = [i as isize, j as isize, k as isize];
                                n[axis] += if side == 0 { -1 } else { 1 };
                                if self.is_liquid(n[0], n[1], n[2]) {
                                    around += self.pressure
                                        [self.cell(n[0] as usize, n[1] as usize, n[2] as usize)];
                                }
                            }
                            let gauss = (rhs[c] + around) / diag[c];
                            self.pressure[c] += omega * (gauss - self.pressure[c]);
                        }
                    }
                }
            }
        }
        // subtract the pressure gradient from every face between cells that are not solid and where at least one holds liquid
        let scale = dt / h;
        for a in 0..3 {
            let fd = self.face_dims(a);
            for k in 0..fd[2] {
                for j in 0..fd[1] {
                    for i in 0..fd[0] {
                        let here = [i as isize, j as isize, k as isize];
                        let mut behind = here;
                        behind[a] -= 1;
                        if self.is_solid(here[0], here[1], here[2])
                            || self.is_solid(behind[0], behind[1], behind[2])
                        {
                            continue;
                        }
                        let (l_hi, l_lo) = (
                            self.is_liquid(here[0], here[1], here[2]),
                            self.is_liquid(behind[0], behind[1], behind[2]),
                        );
                        if !l_hi && !l_lo {
                            continue;
                        }
                        let p_hi = if l_hi {
                            self.pressure[self.cell(i, j, k)]
                        } else {
                            0.0
                        };
                        let p_lo = if l_lo {
                            self.pressure[self.cell(
                                (behind[0]) as usize,
                                behind[1] as usize,
                                behind[2] as usize,
                            )]
                        } else {
                            0.0
                        };
                        let f = self.face(a, i, j, k);
                        self.u[a][f] -= scale * (p_hi - p_lo);
                    }
                }
            }
        }
    }
}
