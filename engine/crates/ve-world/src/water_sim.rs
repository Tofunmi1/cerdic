//! Conservative linear shallow-water solver on a staggered grid, in metres and seconds.
pub mod rain;
mod sample;
pub mod volume;

/// Physical constants and boundary treatment.
#[derive(Clone, Copy, Debug)]
pub struct SimOptions {
    /// Acceleration due to gravity, positive metres per second squared.
    pub gravity: f32,
    /// Velocity damping per second, nonnegative.
    pub damping: f32,
    /// Smooth absorbing border sponge instead of a reflecting outer wall.
    pub absorb_edges: bool,
}

/// Invalid simulation construction input.
#[derive(Debug, thiserror::Error)]
pub enum SimError {
    /// Dimensions must be nonzero and match the supplied cells.
    #[error("grid {width} by {depth} does not match {len} depths")]
    BadShape {
        /// Grid width.
        width: usize,
        /// Grid depth.
        depth: usize,
        /// Supplied depth count.
        len: usize,
    },
    /// A physical parameter was non-finite or outside its domain.
    #[error("invalid water simulation parameter: {name}")]
    BadParameter {
        /// Invalid parameter name.
        name: &'static str,
    },
}

/// Cell-centred displacement and face-centred depth-weighted velocities.
/// The origin is the lower corner; cell centres are at `(i + 0.5) * cell`.
pub struct WaterSim {
    width: usize,
    depth: usize,
    cell: f32,
    depths: Vec<f32>,
    eta: Vec<f32>,
    u: Vec<f32>,
    v: Vec<f32>,
    sponge: Vec<f32>,
    options: SimOptions,
    max_step: f64,
    time: f64,
}

impl WaterSim {
    /// Builds still water; zero depths are impermeable land.
    pub fn new(
        depths: &[f32],
        width: usize,
        depth: usize,
        cell: f32,
        options: SimOptions,
    ) -> Result<Self, SimError> {
        if width == 0 || depth == 0 || width.checked_mul(depth) != Some(depths.len()) {
            return Err(SimError::BadShape {
                width,
                depth,
                len: depths.len(),
            });
        }
        for (name, valid) in [
            ("cell", cell.is_finite() && cell > 0.0),
            (
                "gravity",
                options.gravity.is_finite() && options.gravity > 0.0,
            ),
            (
                "damping",
                options.damping.is_finite() && options.damping >= 0.0,
            ),
            ("depths", depths.iter().all(|h| h.is_finite() && *h >= 0.0)),
        ] {
            if !valid {
                return Err(SimError::BadParameter { name });
            }
        }
        let max_depth = depths.iter().copied().fold(0.0_f32, f32::max);
        let max_step = if max_depth > 0.0 {
            0.45 * f64::from(cell)
                / (2.0 * f64::from(options.gravity) * f64::from(max_depth)).sqrt()
        } else {
            f64::INFINITY
        };
        let mut sponge = vec![0.0; depths.len()];
        if options.absorb_edges {
            for z in 0..depth {
                for x in 0..width {
                    let edge = x.min(z).min(width - 1 - x).min(depth - 1 - z);
                    #[expect(
                        clippy::cast_precision_loss,
                        reason = "distance capped to eight cells"
                    )]
                    let t = 1.0 - edge.min(8) as f32 / 8.0;
                    sponge[z * width + x] = 12.0 * t * t;
                }
            }
        }
        Ok(Self {
            width,
            depth,
            cell,
            depths: depths.to_vec(),
            eta: vec![0.0; depths.len()],
            u: vec![0.0; depths.len()],
            v: vec![0.0; depths.len()],
            sponge,
            options,
            max_step,
            time: 0.0,
        })
    }

    /// Advances by any positive finite duration using CFL-limited symplectic substeps.
    pub fn step(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let mut remaining = f64::from(dt);
        while remaining > 0.0 {
            let sub = remaining.min(self.max_step);
            #[expect(
                clippy::cast_possible_truncation,
                reason = "substep bounded by caller's finite f32 duration"
            )]
            self.substep(sub as f32);
            remaining -= sub;
        }
        self.time += f64::from(dt);
    }

    fn substep(&mut self, dt: f32) {
        let a = dt / self.cell;
        for z in 0..self.depth {
            for x in 0..self.width {
                let i = z * self.width + x;
                if x + 1 < self.width {
                    let h = face(self.depths[i], self.depths[i + 1]);
                    let decay = (-dt
                        * (self.options.damping + self.sponge[i].max(self.sponge[i + 1])))
                    .exp();
                    self.u[i] = (self.u[i]
                        - a * self.options.gravity * h * (self.eta[i + 1] - self.eta[i]))
                        * decay;
                }
                if z + 1 < self.depth {
                    let h = face(self.depths[i], self.depths[i + self.width]);
                    let decay = (-dt
                        * (self.options.damping + self.sponge[i].max(self.sponge[i + self.width])))
                    .exp();
                    self.v[i] = (self.v[i]
                        - a * self.options.gravity * h * (self.eta[i + self.width] - self.eta[i]))
                        * decay;
                }
            }
        }
        for z in 0..self.depth {
            for x in 0..self.width {
                let i = z * self.width + x;
                let left = if x > 0 { self.u[i - 1] } else { 0.0 };
                let back = if z > 0 { self.v[i - self.width] } else { 0.0 };
                self.eta[i] = (self.eta[i] - a * ((self.u[i] - left) + (self.v[i] - back)))
                    * (-dt * self.sponge[i]).exp();
            }
        }
    }

    /// Row-major displacement in metres, suitable for texture upload.
    pub fn heights(&self) -> &[f32] {
        &self.eta
    }

    /// Signed displaced volume in cubic metres.
    pub fn total_volume(&self) -> f64 {
        self.eta.iter().map(|h| f64::from(*h)).sum::<f64>() * f64::from(self.cell).powi(2)
    }
    /// Total elapsed simulation seconds.
    pub fn time(&self) -> f64 {
        self.time
    }

    /// Grid spacing in metres.
    pub fn cell(&self) -> f32 {
        self.cell
    }

    /// Number of cells containing water rather than land.
    pub fn wet_cells(&self) -> usize {
        self.depths.iter().filter(|depth| **depth > 0.0).count()
    }
}

fn face(a: f32, b: f32) -> f32 {
    if a > 0.0 && b > 0.0 {
        a * 0.5 + b * 0.5
    } else {
        0.0
    }
}
