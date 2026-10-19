//! Deterministic procedural audio synthesis, WAV encoding, and spatial maths.
//!
//! Synthesis avoids platform libm trigonometric, exponential, and power functions. Oscillator
//! sine uses a fixed polynomial after range reduction; all variation uses `SplitMix64`. This keeps
//! the synthesis path reproducible across native and wasm32 targets.

use serde::{Deserialize, Serialize};

mod dsp;
mod music;
mod spatial;
use dsp::{PI2, Rng, normalise, semitone, sin, wave};
pub use spatial::{Spatial, Vec3, doppler_ratio, spatialise, to_wav_bytes};

/// Default sample rate used by generated sounds.
pub const DEFAULT_SAMPLE_RATE: u32 = 48_000;

/// Mono floating point PCM with a finite peak no greater than one.
#[derive(Clone, Debug, PartialEq)]
pub struct Sound {
    /// Samples per second.
    pub sample_rate: u32,
    /// Mono samples in the range -1.0 to 1.0.
    pub samples: Vec<f32>,
}
impl Sound {
    fn new(sample_rate: u32, samples: Vec<f32>) -> Self {
        Self {
            sample_rate,
            samples: normalise(samples),
        }
    }
}

/// Periodic oscillator shape or deterministic noise.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Waveform {
    /// Sinusoid.
    Sine,
    /// Rising ramp.
    Saw,
    /// Two-level pulse.
    Square,
    /// Linear triangle.
    Triangle,
    /// White noise.
    WhiteNoise,
}

/// ADSR amplitude envelope in seconds and sustain level.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Envelope {
    /// Attack duration.
    pub attack: f32,
    /// Decay duration.
    pub decay: f32,
    /// Sustain amplitude.
    pub sustain: f32,
    /// Release duration.
    pub release: f32,
}
impl Default for Envelope {
    fn default() -> Self {
        Self {
            attack: 0.01,
            decay: 0.08,
            sustain: 0.7,
            release: 0.12,
        }
    }
}
impl Envelope {
    fn value(self, t: f32, duration: f32) -> f32 {
        let a = self.attack.max(0.0);
        let d = self.decay.max(0.0);
        let r = self.release.max(0.0);
        let end = (duration - r).max(0.0);
        if t < a && a > 0.0 {
            t / a
        } else if t < a + d && d > 0.0 {
            1.0 + (self.sustain.clamp(0.0, 1.0) - 1.0) * ((t - a) / d)
        } else if t < end {
            self.sustain.clamp(0.0, 1.0)
        } else if r > 0.0 {
            self.sustain.clamp(0.0, 1.0) * ((duration - t) / r).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

/// Simple stateful one-pole filter.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Filter {
    /// Filter response.
    pub kind: FilterKind,
    /// Cutoff in Hz.
    pub cutoff_hz: f32,
}
/// Supported filter responses.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum FilterKind {
    /// Passes low frequencies.
    LowPass,
    /// Passes high frequencies.
    HighPass,
    /// Approximate band pass via cascaded high and low pass.
    BandPass,
}

/// Low frequency oscillator configuration.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Lfo {
    /// Frequency in Hz.
    pub rate_hz: f32,
    /// Modulation depth.
    pub depth: f32,
}

/// One oscillator layer in a patch.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Oscillator {
    /// Waveform.
    pub waveform: Waveform,
    /// Relative semitone detune.
    pub detune_semitones: f32,
    /// Linear mix level.
    pub mix: f32,
}
/// Serializable synthesizer patch.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Patch {
    /// Oscillator layers.
    pub oscillators: Vec<Oscillator>,
    /// Noise contribution.
    pub noise_mix: f32,
    /// Optional filter.
    pub filter: Option<Filter>,
    /// Filter cutoff modulation envelope amount, in Hz.
    pub cutoff_envelope: f32,
    /// Amplitude envelope.
    pub amplitude: Envelope,
    /// Pitch sweep in semitones over note duration.
    pub pitch_sweep_semitones: f32,
    /// Output sample rate.
    pub sample_rate: u32,
}
impl Default for Patch {
    fn default() -> Self {
        Self {
            oscillators: vec![Oscillator {
                waveform: Waveform::Sine,
                detune_semitones: 0.0,
                mix: 1.0,
            }],
            noise_mix: 0.0,
            filter: None,
            cutoff_envelope: 0.0,
            amplitude: Envelope::default(),
            pitch_sweep_semitones: 0.0,
            sample_rate: DEFAULT_SAMPLE_RATE,
        }
    }
}

/// Render a patch at the requested pitch and duration.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "bounded sample counts and sample-clock conversions intentionally use f32 arithmetic"
)]
pub fn render(patch: &Patch, note_hz: f32, seconds: f32, seed: u64) -> Sound {
    let rate = patch.sample_rate.max(1);
    let count = (seconds.max(0.0) * rate as f32) as usize;
    let mut rng = Rng(seed);
    let mut phase = vec![0.0_f32; patch.oscillators.len()];
    let mut low = 0.0;
    let mut band = 0.0;
    let mut out = Vec::with_capacity(count);
    let duration = count as f32 / rate as f32;
    for i in 0..count {
        let t = i as f32 / rate as f32;
        let progress = if duration > 0.0 { t / duration } else { 0.0 };
        let hz = note_hz.max(0.0) * semitone(patch.pitch_sweep_semitones * progress);
        let mut value = rng.noise() * patch.noise_mix;
        for (n, osc) in patch.oscillators.iter().enumerate() {
            let f = hz * semitone(osc.detune_semitones);
            phase[n] = (phase[n] + f / rate as f32).fract();
            value += wave(osc.waveform, phase[n], &mut rng) * osc.mix;
        }
        if let Some(filter) = patch.filter {
            let cutoff = (filter.cutoff_hz
                + patch.cutoff_envelope * patch.amplitude.value(t, duration))
            .clamp(1.0, rate as f32 * 0.45);
            let alpha = (cutoff / rate as f32).clamp(0.0, 0.45);
            low += alpha * (value - low);
            let hp = value - low;
            band += alpha * (hp - band);
            value = match filter.kind {
                FilterKind::LowPass => low,
                FilterKind::HighPass => hp,
                FilterKind::BandPass => band,
            };
        }
        out.push(value * patch.amplitude.value(t, duration));
    }
    Sound::new(rate, out)
}

/// Procedural sound effect recipes.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SfxKind {
    /// Collectible pickup.
    Coin,
    /// Jump.
    Jump,
    /// Impact.
    Hit,
    /// Large blast.
    Explosion,
    /// Sci-fi zap.
    Laser,
    /// Stone footfall.
    FootstepStone,
    /// Wooden footfall.
    FootstepWood,
    /// Grass footfall.
    FootstepGrass,
    /// Metal footfall.
    FootstepMetal,
    /// Air movement.
    Whoosh,
    /// UI click.
    Click,
    /// Rising reward.
    PowerUp,
    /// Water splash.
    Splash,
    /// Door opening.
    DoorOpen,
    /// Door closing.
    DoorClose,
    /// Sustained wind.
    WindLoop,
    /// Sustained rain.
    RainLoop,
}
/// Rendered effect kind list is exhaustive for callers and tests.
pub const ALL_SFX: [SfxKind; 17] = [
    SfxKind::Coin,
    SfxKind::Jump,
    SfxKind::Hit,
    SfxKind::Explosion,
    SfxKind::Laser,
    SfxKind::FootstepStone,
    SfxKind::FootstepWood,
    SfxKind::FootstepGrass,
    SfxKind::FootstepMetal,
    SfxKind::Whoosh,
    SfxKind::Click,
    SfxKind::PowerUp,
    SfxKind::Splash,
    SfxKind::DoorOpen,
    SfxKind::DoorClose,
    SfxKind::WindLoop,
    SfxKind::RainLoop,
];
/// Generate one sound effect. Intensity is clamped to 0..1 and scales loudness and brightness.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "fixed short recipe durations are converted to sample counts and sample-clock f32 values"
)]
pub fn sfx(kind: SfxKind, seed: u64, intensity: f32) -> Sound {
    let mut rng = Rng(seed);
    let intensity = intensity.clamp(0.0, 1.0);
    let (seconds, base, noisy) = match kind {
        SfxKind::Coin => (0.22, 700.0, false),
        SfxKind::Jump => (0.32, 180.0, false),
        SfxKind::Hit => (0.24, 90.0, true),
        SfxKind::Explosion => (0.9, 55.0, true),
        SfxKind::Laser => (0.38, 500.0, false),
        SfxKind::FootstepStone => (0.16, 100.0, true),
        SfxKind::FootstepWood => (0.18, 140.0, true),
        SfxKind::FootstepGrass => (0.2, 220.0, true),
        SfxKind::FootstepMetal => (0.2, 330.0, true),
        SfxKind::Whoosh => (0.55, 300.0, true),
        SfxKind::Click => (0.06, 900.0, false),
        SfxKind::PowerUp => (0.75, 260.0, false),
        SfxKind::Splash => (0.42, 180.0, true),
        SfxKind::DoorOpen => (0.65, 85.0, true),
        SfxKind::DoorClose => (0.5, 75.0, true),
        SfxKind::WindLoop => (2.0, 100.0, true),
        SfxKind::RainLoop => (2.0, 450.0, true),
    };
    let count = (seconds * DEFAULT_SAMPLE_RATE as f32) as usize;
    let base = base * (0.96 + rng.noise().abs() * 0.08);
    let mut phase = 0.0;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let t = i as f32 / count as f32;
        let sweep = 1.0 + (1.0 - t) * intensity * 2.0;
        phase = (phase + base * sweep / DEFAULT_SAMPLE_RATE as f32).fract();
        let env = if matches!(kind, SfxKind::WindLoop | SfxKind::RainLoop) {
            0.55 + 0.45 * sin(PI2 * t).abs()
        } else {
            (1.0 - t).max(0.0).powi(2)
        };
        let tone = sin(phase * PI2);
        let n = if noisy { rng.noise() * intensity } else { 0.0 };
        out.push((tone * (1.0 - intensity * 0.35) + n) * env * intensity * 0.8);
    }
    Sound::new(DEFAULT_SAMPLE_RATE, out)
}

pub use music::{Mood, MusicSpec, Scale, music};
