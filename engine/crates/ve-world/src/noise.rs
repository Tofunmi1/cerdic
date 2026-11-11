//! Coherent noise in two to four dimensions: one implementation for terrain, scatter, effects and anything else that needs it.
//!
//! A [`Noise`] is a basis (value, Perlin, simplex or cellular), an optional fractal over it (fBm, ridged, billow) and an optional
//! domain warp. Every sample is in `[-1, 1]`, deterministic, and uses integer hashes and `+ - * /` with `sqrt` only. The fourth
//! dimension is time, for water, clouds and fire. The structure follows
//! `FastNoise2` (MIT, Jordan Peck).

mod cellular;
mod hash;
mod lattice;
mod simplex;

use thiserror::Error;

/// Most octaves a fractal or a warp may use.
pub const MAX_OCTAVES: u8 = 12;

/// How cellular noise measures distance to a feature point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellDistance {
    /// Straight-line distance: round cells.
    Euclidean,
    /// Sum of axis distances: diamond cells.
    Manhattan,
    /// Squared straight-line distance plus the Manhattan distance: soft-cornered cells.
    Hybrid,
}

/// What cellular noise returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellReturn {
    /// A random value per cell: flat patches.
    Value,
    /// Distance to the nearest feature point: dark at each point, bright towards the borders.
    Distance,
    /// Distance to the second-nearest feature point.
    Distance2,
    /// Second minus nearest distance: zero on the cell borders, so it draws the borders.
    Difference,
}

/// The base pattern.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Basis {
    /// Smoothly interpolated random values on the lattice.
    Value,
    /// Gradient noise (improved Perlin): zero on every lattice point, no blocky look.
    Perlin,
    /// Simplex noise: cheaper in 4D and with fewer axis-aligned artefacts than Perlin.
    Simplex,
    /// Worley noise.
    Cellular {
        /// Distance measure.
        distance: CellDistance,
        /// What to return.
        ret: CellReturn,
        /// How far each feature point may move from its cell centre, `0` (a regular grid) to `1` (anywhere in the cell).
        jitter: f64,
    },
}

/// How octaves combine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FractalKind {
    /// Plain sum of octaves.
    Fbm,
    /// `1 - 2|n|` per octave: sharp creases, for mountain ridges.
    Ridged,
    /// `2|n| - 1` per octave: soft rounded humps, for clouds.
    Billow,
}

/// Several octaves of the basis at rising frequency and falling amplitude.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fractal {
    /// How octaves combine.
    pub kind: FractalKind,
    /// Number of octaves, `1..=12`.
    pub octaves: u8,
    /// Frequency multiplier per octave, `1..=4`.
    pub lacunarity: f64,
    /// Amplitude multiplier per octave, `0..=1`.
    pub gain: f64,
    /// `0..=1`: how much a low octave value damps the next octave, so valleys stay smooth and peaks get detail.
    pub weighted_strength: f64,
}

/// Which noise moves the coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WarpKind {
    /// Perlin offsets.
    Gradient,
    /// Simplex offsets.
    Simplex,
}

/// A domain warp: each coordinate is moved by a noise before sampling, which bends straight features into natural ones.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Warp {
    /// Which noise supplies the offsets.
    pub kind: WarpKind,
    /// Largest offset, in noise units (after the main frequency); `0` or more.
    pub amplitude: f64,
    /// Frequency of the offset noise relative to the main frequency; above `0`.
    pub frequency: f64,
    /// Warp passes, `1..=12`; each is half the amplitude and double the frequency of the one before.
    pub octaves: u8,
}

/// A refused setting.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum NoiseError {
    /// A setting is outside its range.
    #[error("noise {name}: {why}")]
    Setting {
        /// The setting.
        name: &'static str,
        /// What is allowed.
        why: &'static str,
    },
}

/// A noise function.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Noise {
    /// The base pattern.
    pub basis: Basis,
    /// Different seeds give unrelated patterns.
    pub seed: u32,
    /// Lattice cells per unit of input; above `0`.
    pub frequency: f64,
    /// Optional fractal over the basis.
    pub fractal: Option<Fractal>,
    /// Optional domain warp.
    pub warp: Option<Warp>,
}

impl Default for Noise {
    fn default() -> Self {
        Self {
            basis: Basis::Perlin,
            seed: 0,
            frequency: 1.0,
            fractal: None,
            warp: None,
        }
    }
}

fn check(ok: bool, name: &'static str, why: &'static str) -> Result<(), NoiseError> {
    if ok {
        Ok(())
    } else {
        Err(NoiseError::Setting { name, why })
    }
}

fn octaves_ok(octaves: u8) -> Result<(), NoiseError> {
    check(
        (1..=MAX_OCTAVES).contains(&octaves),
        "octaves",
        "use 1 to 12 octaves",
    )
}

impl Noise {
    /// Checks every setting.
    ///
    /// # Errors
    /// Names the first setting that is out of range.
    pub fn validate(&self) -> Result<(), NoiseError> {
        check(
            self.frequency.is_finite() && self.frequency > 0.0,
            "frequency",
            "use a finite frequency above 0",
        )?;
        if let Basis::Cellular { jitter, .. } = self.basis {
            check(
                (0.0..=1.0).contains(&jitter),
                "jitter",
                "use a jitter from 0 to 1",
            )?;
        }
        if let Some(f) = &self.fractal {
            octaves_ok(f.octaves)?;
            check(
                (1.0..=4.0).contains(&f.lacunarity),
                "lacunarity",
                "use a lacunarity from 1 to 4",
            )?;
            check(
                (0.0..=1.0).contains(&f.gain),
                "gain",
                "use a gain from 0 to 1",
            )?;
            check(
                (0.0..=1.0).contains(&f.weighted_strength),
                "weighted_strength",
                "use a weighted strength from 0 to 1",
            )?;
        }
        if let Some(w) = &self.warp {
            octaves_ok(w.octaves)?;
            check(
                w.amplitude.is_finite() && w.amplitude >= 0.0,
                "amplitude",
                "use a finite amplitude of 0 or more",
            )?;
            check(
                w.frequency.is_finite() && w.frequency > 0.0,
                "frequency",
                "use a warp frequency above 0",
            )?;
        }
        Ok(())
    }

    /// A sample in `[-1, 1]`; `0` where any coordinate is not finite.
    #[must_use]
    pub fn sample2(&self, x: f64, y: f64) -> f64 {
        self.sample([x, y, 0.0, 0.0], 2)
    }

    /// A sample in `[-1, 1]`; `0` where any coordinate is not finite.
    #[must_use]
    pub fn sample3(&self, x: f64, y: f64, z: f64) -> f64 {
        self.sample([x, y, z, 0.0], 3)
    }

    /// A sample in `[-1, 1]`; `w` is time. `0` where any coordinate is not finite.
    #[must_use]
    pub fn sample4(&self, x: f64, y: f64, z: f64, w: f64) -> f64 {
        self.sample([x, y, z, w], 4)
    }

    fn sample(&self, mut p: [f64; 4], dims: usize) -> f64 {
        if p[..dims].iter().any(|c| !c.is_finite()) {
            return 0.0;
        }
        for c in &mut p[..dims] {
            *c *= self.frequency;
        }
        if let Some(warp) = &self.warp {
            p = self.warped(warp, p, dims);
        }
        let value = match &self.fractal {
            None => self.basis_at(self.seed, &p, dims),
            Some(fractal) => self.fractal_at(fractal, &p, dims),
        };
        if value.is_finite() {
            value.clamp(-1.0, 1.0)
        } else {
            0.0
        }
    }

    fn basis_at(&self, seed: u32, p: &[f64; 4], dims: usize) -> f64 {
        match self.basis {
            Basis::Value => lattice::value(seed, p, dims),
            Basis::Perlin => lattice::perlin(seed, p, dims),
            Basis::Simplex => simplex::simplex(seed, p, dims),
            Basis::Cellular {
                distance,
                ret,
                jitter,
            } => cellular::cellular(seed, p, dims, distance, ret, jitter),
        }
    }

    fn fractal_at(&self, fractal: &Fractal, p: &[f64; 4], dims: usize) -> f64 {
        let (mut sum, mut bound) = (0.0, 0.0);
        let (mut amplitude, mut unweighted, mut scale) = (1.0, 1.0, 1.0);
        for octave in 0..fractal.octaves {
            let mut at = *p;
            for c in &mut at[..dims] {
                *c *= scale;
            }
            let raw = self.basis_at(self.seed.wrapping_add(u32::from(octave)), &at, dims);
            let sample = match fractal.kind {
                FractalKind::Fbm => raw,
                FractalKind::Ridged => 1.0 - 2.0 * raw.abs(),
                FractalKind::Billow => 2.0 * raw.abs() - 1.0,
            };
            sum += sample * amplitude;
            bound += unweighted;
            let damp = 1.0 + ((sample + 1.0) * 0.5 - 1.0) * fractal.weighted_strength;
            amplitude *= fractal.gain * damp;
            unweighted *= fractal.gain;
            scale *= fractal.lacunarity;
        }
        sum / bound
    }

    fn warped(&self, warp: &Warp, mut p: [f64; 4], dims: usize) -> [f64; 4] {
        let (mut amplitude, mut frequency) = (warp.amplitude, warp.frequency);
        for octave in 0..u32::from(warp.octaves) {
            let mut moved = [0.0; 4];
            for (axis, slot) in moved.iter_mut().enumerate().take(dims) {
                let seed = self.seed
                    ^ 0xA511_E9B3_u32.wrapping_mul(octave + 1)
                    ^ 0x63D8_3595_u32.wrapping_mul(u32::try_from(axis).unwrap_or(0) + 1);
                let mut at = p;
                for c in &mut at[..dims] {
                    *c *= frequency;
                }
                let offset = match warp.kind {
                    WarpKind::Gradient => lattice::perlin(seed, &at, dims),
                    WarpKind::Simplex => simplex::simplex(seed, &at, dims),
                };
                *slot = offset.clamp(-1.0, 1.0) * amplitude;
            }
            for (c, delta) in p.iter_mut().zip(moved).take(dims) {
                *c += delta;
            }
            amplitude *= 0.5;
            frequency *= 2.0;
        }
        p
    }
}
