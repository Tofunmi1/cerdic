//! Deterministic travelling waves with deep-water dispersion, shared with the forward shader.

use std::f32::consts::TAU;

/// Acceleration due to gravity in metres per second squared.
pub const GRAVITY: f32 = 9.81;
/// Number of waves in the material's primary wave table.
pub const WAVES: usize = 12;

/// One travelling sine wave; direction zero travels along positive X.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wave {
    /// Travel direction in degrees on the XZ plane.
    pub direction_deg: f32,
    /// Wavelength in metres.
    pub wavelength: f32,
    /// Height amplitude in metres.
    pub amplitude: f32,
    /// Initial phase in radians.
    pub phase: f32,
}

/// Builds a repeatable swell-led spread from 0.3 to 12 metres, centred on direction zero.
#[must_use]
pub fn wave_table(seed: u32) -> [Wave; WAVES] {
    let mut state = seed ^ 0xa511_e9b3;
    let mut random = || {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        f32::from(u16::try_from(state >> 16).unwrap_or(0)) / 65_535.0
    };
    std::array::from_fn(|i| {
        let fraction = f32::from(u16::try_from(i).unwrap_or(0)) / 11.0;
        let wavelength = (0.3 * 40.0_f32.powf(fraction)).clamp(0.3, 12.0);
        Wave {
            direction_deg: random() * 140.0 - 70.0,
            wavelength,
            // Long waves carry the height; sub-metre waves add only a little ripple.
            // At ripple 1 the sum stays below 0.15 m and individual slopes below 0.08.
            amplitude: (0.002_05 * wavelength.powf(1.3) * (wavelength / 0.8).min(1.0))
                .min(0.08 * wavelength / TAU),
            phase: random() * TAU,
        }
    })
}

/// A bounded wave field whose analytic normal is the gradient of its height.
#[derive(Clone, Debug)]
pub struct WaveField {
    waves: Vec<Wave>,
    flow: f32,
}

pub(crate) fn finite_or(value: f32, default: f32) -> f32 {
    if value.is_finite() { value } else { default }
}

impl WaveField {
    /// Builds a seeded field. Ripple is clamped to 0..2 and flow to 0..4; non-finite
    /// values use ripple 1, flow 1 and direction 0. Ripple scales amplitudes linearly.
    #[must_use]
    pub fn new(seed: u32, ripple: f32, flow: f32, direction_deg: f32) -> Self {
        let ripple = finite_or(ripple, 1.0).clamp(0.0, 2.0);
        let direction = finite_or(direction_deg, 0.0).rem_euclid(360.0);
        let waves = wave_table(seed).map(|mut wave| {
            wave.amplitude *= ripple;
            wave.direction_deg += direction;
            wave
        });
        Self::from_waves(&waves, flow)
    }

    /// Builds a reference field from explicit waves, skipping non-finite waves and
    /// non-positive wavelengths. Flow uses the same defaults and bounds as [`Self::new`].
    #[must_use]
    pub fn from_waves(waves: &[Wave], flow: f32) -> Self {
        Self {
            waves: waves
                .iter()
                .copied()
                .filter(|w| {
                    w.direction_deg.is_finite()
                        && w.wavelength.is_finite()
                        && w.wavelength > 0.0
                        && w.amplitude.is_finite()
                        && w.phase.is_finite()
                })
                .collect(),
            flow: finite_or(flow, 1.0).clamp(0.0, 4.0),
        }
    }

    /// The exact waves evaluated by this field, after amplitude and direction adjustment.
    #[must_use]
    pub fn waves(&self) -> &[Wave] {
        &self.waves
    }

    /// Height in metres at ground-plane position `(x, z)` and time `t` in seconds.
    #[must_use]
    pub fn height(&self, x: f32, z: f32, t: f32) -> f32 {
        self.waves
            .iter()
            .map(|wave| {
                let (_, _, phase) = self.coordinates(wave, x, z, t);
                wave.amplitude * phase.sin()
            })
            .sum()
    }

    /// Upward-facing unit normal from the analytic spatial derivatives of the height.
    #[must_use]
    pub fn normal(&self, x: f32, z: f32, t: f32) -> [f32; 3] {
        let (mut dx, mut dz) = (0.0, 0.0);
        for wave in &self.waves {
            let (kx, kz, phase) = self.coordinates(wave, x, z, t);
            let slope = wave.amplitude * phase.cos();
            dx += slope * kx;
            dz += slope * kz;
        }
        let length = (dx * dx + 1.0 + dz * dz).sqrt();
        [-dx / length, 1.0 / length, -dz / length]
    }

    /// Conservative absolute height bound, the sum of absolute amplitudes.
    #[must_use]
    pub fn max_height(&self) -> f32 {
        self.waves.iter().map(|wave| wave.amplitude.abs()).sum()
    }

    fn coordinates(&self, wave: &Wave, x: f32, z: f32, t: f32) -> (f32, f32, f32) {
        let k = TAU / wave.wavelength;
        let (dz, dx) = wave.direction_deg.to_radians().sin_cos();
        let (kx, kz) = (k * dx, k * dz);
        let omega = (GRAVITY * k).sqrt() * self.flow;
        (kx, kz, kx * x + kz * z - omega * t + wave.phase)
    }
}
