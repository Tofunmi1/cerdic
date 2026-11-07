//! What cloth, rope and hair collide with: spheres, capsules (a bone), planes and signed-distance grids (any closed mesh).
//!
//! Every collider answers one question: how far outside it is a point, and which way is out. Arithmetic and `sqrt` only.

use super::V3;

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn scale(a: V3, s: f32) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

/// A grid of signed distances (negative inside) on a regular lattice, sampled with trilinear interpolation.
#[derive(Clone, Debug)]
pub struct SdfGrid {
    /// World position of voxel (0, 0, 0).
    pub origin: V3,
    /// Voxel size in metres.
    pub cell: f32,
    /// Voxels along x, y and z.
    pub dims: [usize; 3],
    /// Distances, x fastest then y then z.
    pub values: Vec<f32>,
}

impl SdfGrid {
    fn at(&self, i: usize, j: usize, k: usize) -> f32 {
        self.values[i + self.dims[0] * (j + self.dims[1] * k)]
    }

    /// A grid sampled only near the surface holds the clamped value `trunc` farther out, which has no direction: a particle pressed deeper
    /// than the band would feel no push. This replaces every clamped value by a true distance grown outward from the exact band (fast
    /// sweeping of the Eikonal equation, Zhao 2005), keeping each voxel's sign, so every point has a way out.
    pub fn fill_far(&mut self, trunc: f32) {
        let [nx, ny, nz] = self.dims;
        if nx < 2 || ny < 2 || nz < 2 || self.values.len() < nx * ny * nz {
            return;
        }
        let exact: Vec<bool> = self
            .values
            .iter()
            .map(|v| v.abs() < trunc * 0.999)
            .collect();
        let sign: Vec<f32> = self
            .values
            .iter()
            .map(|&v| if v < 0.0 { -1.0 } else { 1.0 })
            .collect();
        let mut u: Vec<f32> = self
            .values
            .iter()
            .zip(&exact)
            .map(|(v, &e)| if e { v.abs() } else { f32::INFINITY })
            .collect();
        let h = self.cell;
        for _ in 0..2 {
            for order in 0..8_u32 {
                let (rx, ry, rz) = (order & 1 != 0, order & 2 != 0, order & 4 != 0);
                for kk in 0..nz {
                    let k = if rz { nz - 1 - kk } else { kk };
                    for jj in 0..ny {
                        let j = if ry { ny - 1 - jj } else { jj };
                        for ii in 0..nx {
                            let i = if rx { nx - 1 - ii } else { ii };
                            let at = i + nx * (j + ny * k);
                            if exact[at] {
                                continue;
                            }
                            let pick = |a: Option<f32>, b: Option<f32>| match (a, b) {
                                (Some(x), Some(y)) => x.min(y),
                                (Some(x), None) | (None, Some(x)) => x,
                                (None, None) => f32::INFINITY,
                            };
                            let get = |di: isize, dj: isize, dk: isize| {
                                let (a, b, c) = (i as isize + di, j as isize + dj, k as isize + dk);
                                (a >= 0
                                    && b >= 0
                                    && c >= 0
                                    && (a as usize) < nx
                                    && (b as usize) < ny
                                    && (c as usize) < nz)
                                    .then(|| u[a as usize + nx * (b as usize + ny * c as usize)])
                            };
                            let mut m = [
                                pick(get(-1, 0, 0), get(1, 0, 0)),
                                pick(get(0, -1, 0), get(0, 1, 0)),
                                pick(get(0, 0, -1), get(0, 0, 1)),
                            ];
                            m.sort_by(f32::total_cmp);
                            let [a, b, c] = m;
                            if !a.is_finite() {
                                continue;
                            }
                            let mut candidate = a + h;
                            if b.is_finite() && candidate > b {
                                candidate =
                                    (a + b + (2.0 * h * h - (a - b) * (a - b)).max(0.0).sqrt())
                                        * 0.5;
                                if c.is_finite() && candidate > c {
                                    let sum = a + b + c;
                                    let discriminant = (sum * sum
                                        - 3.0 * (a * a + b * b + c * c - h * h))
                                        .max(0.0);
                                    candidate = (sum + discriminant.sqrt()) / 3.0;
                                }
                            }
                            if candidate < u[at] {
                                u[at] = candidate;
                            }
                        }
                    }
                }
            }
        }
        for (at, v) in self.values.iter_mut().enumerate() {
            if !exact[at] && u[at].is_finite() {
                *v = sign[at] * u[at];
            }
        }
    }

    /// Distance and outward direction at `p`; outside the grid the distance is the grid's edge value plus the way to the box, directed away from it.
    fn sample(&self, p: V3) -> (f32, V3) {
        if self.dims.iter().any(|&d| d < 2)
            || self.values.len() < self.dims[0] * self.dims[1] * self.dims[2]
        {
            return (f32::MAX, [0.0, 1.0, 0.0]);
        }
        let g = [0, 1, 2].map(|a| (p[a] - self.origin[a]) / self.cell);
        let clamped = [0, 1, 2].map(|a| g[a].clamp(0.0, (self.dims[a] - 1) as f32));
        let cell_index = [0, 1, 2].map(|a| (clamped[a].floor() as usize).min(self.dims[a] - 2));
        let t = [0, 1, 2].map(|a| clamped[a] - cell_index[a] as f32);
        let corner = |dx: usize, dy: usize, dz: usize| {
            self.at(cell_index[0] + dx, cell_index[1] + dy, cell_index[2] + dz)
        };
        let lerp = |a: f32, b: f32, s: f32| a + (b - a) * s;
        // Trilinear value, and its exact gradient inside the cell.
        let c = [
            [
                [corner(0, 0, 0), corner(0, 0, 1)],
                [corner(0, 1, 0), corner(0, 1, 1)],
            ],
            [
                [corner(1, 0, 0), corner(1, 0, 1)],
                [corner(1, 1, 0), corner(1, 1, 1)],
            ],
        ];
        let x00 = lerp(c[0][0][0], c[1][0][0], t[0]);
        let x01 = lerp(c[0][0][1], c[1][0][1], t[0]);
        let x10 = lerp(c[0][1][0], c[1][1][0], t[0]);
        let x11 = lerp(c[0][1][1], c[1][1][1], t[0]);
        let y0 = lerp(x00, x10, t[1]);
        let y1 = lerp(x01, x11, t[1]);
        let value = lerp(y0, y1, t[2]);
        let gx = lerp(
            lerp(c[1][0][0] - c[0][0][0], c[1][1][0] - c[0][1][0], t[1]),
            lerp(c[1][0][1] - c[0][0][1], c[1][1][1] - c[0][1][1], t[1]),
            t[2],
        );
        let gy = lerp(x10 - x00, x11 - x01, t[2]);
        let gz = y1 - y0;
        let gradient = [gx, gy, gz];
        let length = dot(gradient, gradient).sqrt();
        let normal = if length > 1e-9 {
            scale(gradient, 1.0 / length)
        } else {
            [0.0, 1.0, 0.0]
        };
        // Beyond the box the field is flat: add the distance to the box so points far outside read as far away.
        let outside = [0, 1, 2].map(|a| (g[a] - clamped[a]) * self.cell);
        let reach = dot(outside, outside).sqrt();
        if reach > 0.0 {
            (value + reach, scale(outside, 1.0 / reach))
        } else {
            (value, normal)
        }
    }
}

/// A shape that pushes particles out.
#[derive(Clone, Debug)]
pub enum Collider {
    /// A ball.
    Sphere {
        /// Centre.
        center: V3,
        /// Radius in metres.
        radius: f32,
    },
    /// A capsule: the points within `radius` of the segment from `a` to `b` (a bone with thickness).
    Capsule {
        /// One end of the axis.
        a: V3,
        /// The other end.
        b: V3,
        /// Radius in metres.
        radius: f32,
    },
    /// Everything below the plane through `point` facing `normal` (a floor).
    Plane {
        /// A point on the plane.
        point: V3,
        /// The way out; need not be unit length.
        normal: V3,
    },
    /// Any closed shape sampled as signed distances.
    Sdf(SdfGrid),
}

impl Collider {
    /// How far outside the collider `p` is (negative inside) and the unit direction pointing out.
    #[must_use]
    pub fn distance(&self, p: V3) -> (f32, V3) {
        match self {
            Self::Sphere { center, radius } => {
                let d = sub(p, *center);
                let l = dot(d, d).sqrt();
                if l > 1e-9 {
                    (l - radius, scale(d, 1.0 / l))
                } else {
                    (-radius, [0.0, 1.0, 0.0])
                }
            }
            Self::Capsule { a, b, radius } => {
                let axis = sub(*b, *a);
                let along = dot(axis, axis);
                let t = if along > 1e-12 {
                    (dot(sub(p, *a), axis) / along).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let nearest = [a[0] + axis[0] * t, a[1] + axis[1] * t, a[2] + axis[2] * t];
                let d = sub(p, nearest);
                let l = dot(d, d).sqrt();
                if l > 1e-9 {
                    (l - radius, scale(d, 1.0 / l))
                } else {
                    (-radius, [0.0, 1.0, 0.0])
                }
            }
            Self::Plane { point, normal } => {
                let l = dot(*normal, *normal).sqrt();
                let n = if l > 1e-9 {
                    scale(*normal, 1.0 / l)
                } else {
                    [0.0, 1.0, 0.0]
                };
                (dot(sub(p, *point), n), n)
            }
            Self::Sdf(grid) => grid.sample(p),
        }
    }
}
