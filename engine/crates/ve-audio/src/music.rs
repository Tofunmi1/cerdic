use super::{
    DEFAULT_SAMPLE_RATE, Sound,
    dsp::{PI2, Rng, semitone, sin},
};
use serde::{Deserialize, Serialize};

/// Musical scale choices.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Scale {
    /// Major scale.
    Major,
    /// Natural minor.
    Minor,
    /// Pentatonic major.
    Pentatonic,
}
/// Musical mood controls voicing and rhythm.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Mood {
    /// Gentle and spacious.
    Calm,
    /// Unsettling.
    Tense,
    /// Triumphant.
    Heroic,
    /// Curious.
    Mysterious,
    /// Bright and bouncy.
    Playful,
}
/// Procedural looping music request.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct MusicSpec {
    /// Variation seed.
    pub seed: u64,
    /// Beats per minute.
    pub tempo_bpm: f32,
    /// Number of bars.
    pub bars: u32,
    /// Pitch collection.
    pub scale: Scale,
    /// Arrangement mood.
    pub mood: Mood,
}
/// Generate a seamless loop with an integer number of bars.
#[must_use]
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "music timing and bounded beat indices intentionally map between sample and musical clocks"
)]
pub fn music(spec: MusicSpec) -> Sound {
    let bpm = spec.tempo_bpm.clamp(30.0, 300.0);
    let bars = spec.bars.clamp(1, 1_000);
    let seconds = bars as f32 * 4.0 * 60.0 / bpm;
    let count = (seconds * DEFAULT_SAMPLE_RATE as f32) as usize;
    let mut rng = Rng(spec.seed);
    let mut out = Vec::with_capacity(count);
    let offsets: &[i32] = match spec.scale {
        Scale::Major => &[0, 2, 4, 5, 7, 9, 11],
        Scale::Minor => &[0, 2, 3, 5, 7, 8, 10],
        Scale::Pentatonic => &[0, 2, 4, 7, 9],
    };
    let beat = 60.0 / bpm;
    for i in 0..count {
        let t = i as f32 / DEFAULT_SAMPLE_RATE as f32;
        let beat_pos = t / beat;
        let step = beat_pos.floor() as u32;
        let bar = step / 4;
        let chord = (bar % 4) as usize;
        let degree = match (spec.mood, chord) {
            (Mood::Tense, _) => [0, 1, 4, 5][chord],
            (Mood::Mysterious, _) => [0, 5, 2, 4][chord],
            (_, _) => [0, 4, 5, 3][chord],
        };
        let note = offsets[(degree as usize) % offsets.len()] as f32;
        let low_note = sin((55.0 * semitone(note - 12.0) * t) * PI2);
        let arp_step = (beat_pos * 2.0).floor() as usize;
        let arp_note = offsets[(arp_step + degree as usize) % offsets.len()] as f32;
        let arp = sin((220.0 * semitone(arp_note) * t) * PI2);
        let drum = if step.is_multiple_of(4) {
            rng.noise() * 0.12
        } else {
            0.0
        };
        let seam_width = 0.02_f32.min(seconds * 0.25);
        let seam = (t / seam_width).min(1.0) * ((seconds - t) / seam_width).min(1.0);
        out.push((low_note * 0.24 + arp * 0.15 + drum) * seam);
    }
    Sound::new(DEFAULT_SAMPLE_RATE, out)
}
