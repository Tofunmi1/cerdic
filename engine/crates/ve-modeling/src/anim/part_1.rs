use std::rc::Rc;

use serde::{Deserialize, Serialize};
use ve_graph::{Node, Value as Param};

use crate::{Fail, cache::Value, fail, fail_suggest};

/// Most keys one track may have.
pub const MAX_KEYS: usize = 64;
/// Most tracks one part may carry.
pub const MAX_TRACKS: usize = 16;
/// Longest a clip may run, in seconds.
pub const MAX_DURATION: f32 = 3600.0;

/// What a track changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackProperty {
    /// Translation in metres, added to the part's position.
    Position,
    /// Euler angles in degrees, applied about the pivot in the order Y, X, Z.
    Rotation,
    /// Per-axis scale factors about the pivot (1 is unchanged).
    Scale,
    /// How strongly the part glows: a factor on its emissive colour and on the light it emits (the first
    /// value; 1 is as authored, 0 is dark). Does not move the part.
    Glow,
    /// Whether the part is shown (the first value: 0.5 or more is shown, less is hidden; a hidden part also
    /// stops emitting light). Does not move the part. Pair it with `step` interpolation.
    Visible,
    /// The part's surface colour (red, green, blue in 0 to 1, as authored in `solid.material`). Replaces the
    /// colour the part was authored with while the track plays. Does not move the part.
    Color,
}

/// How a value moves between two keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Interpolation {
    /// Straight line between keys.
    Linear,
    /// Ease in and out (smoothstep) between keys.
    Smooth,
    /// Start slowly and speed up (quadratic) between keys.
    EaseIn,
    /// Start fast and slow down into the next key (quadratic).
    EaseOut,
    /// A smooth curve through all the keys (Blender's automatic clamped Bezier handles): motion flows through
    /// a key instead of stopping at it, never overshoots the keys on either side, and the first and last keys
    /// ease in and out.
    Bezier,
    /// Rushes past the next key a little and returns (Blender's back easing).
    Back,
    /// Bounces against the next key (Blender's bounce easing).
    Bounce,
    /// Springs past the next key and oscillates down onto it (Blender's elastic easing).
    Elastic,
    /// Hold each key's value until the next key.
    Step,
}

/// One keyframe: a time in seconds and a value (metres, degrees or a scale factor per axis).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Key {
    /// Seconds from the start of the clip.
    pub time: f32,
    /// The value at that time.
    pub value: [f32; 3],
}

/// A keyframed transform track on a part.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    /// Which transform component it drives.
    pub property: TrackProperty,
    /// Keys in strictly increasing time order; at least two.
    pub keys: Vec<Key>,
    /// How values move between keys.
    pub interpolation: Interpolation,
    /// Whether the track starts over after its last key.
    pub looped: bool,
    /// The point, in the part's own space, that rotation and scale act about.
    pub pivot: [f32; 3],
    /// The clip this track belongs to ("" is the part's main clip). Clips play separately: a behaviour runs one clip,
    /// and the poses of all of a part's clips combine.
    #[serde(default)]
    pub clip: String,
    /// For a set of placed copies: seconds added to the clock for each copy's index, so copy `i` plays `i * phase_step`
    /// seconds ahead (a wave through a row). Ignored for a single part.
    #[serde(default)]
    pub phase_step: f32,
    /// For a set of placed copies: up to this many extra seconds of a fixed pseudo-random offset per copy, so a field of
    /// them does not move in step. Ignored for a single part.
    #[serde(default)]
    pub phase_jitter: f32,
}

/// Longest phase offset, in seconds.
pub const MAX_PHASE: f32 = 60.0;

/// A fraction in 0 up to (not including) 1 that depends only on the copy's index (a fixed integer mix, so every machine
/// agrees and neighbouring copies do not follow each other).
#[must_use]
pub fn jitter_fraction(index: u32) -> f32 {
    let mut x = index.wrapping_add(0x9e37_79b9);
    x = (x ^ (x >> 16)).wrapping_mul(0x21f0_aaad);
    x = (x ^ (x >> 15)).wrapping_mul(0x735a_2d97);
    x ^= x >> 15;
    #[expect(
        clippy::cast_precision_loss,
        reason = "the top 24 bits fit an f32 exactly"
    )]
    let fraction = (x >> 8) as f32 / 16_777_216.0;
    fraction
}

impl Track {
    /// Seconds copy `index` of a placed set plays ahead of the clock.
    #[must_use]
    #[expect(
        clippy::cast_precision_loss,
        reason = "copy indices are far below 2^24"
    )]
    pub fn phase(&self, index: u32) -> f32 {
        self.phase_step * index as f32 + self.phase_jitter * jitter_fraction(index)
    }
}

/// Reads `phase_step` and `phase_jitter`; they only mean something for a set of placed copies.
pub(crate) fn phases(node: &Node, input: &Value) -> Result<(f32, f32), Fail> {
    #[expect(clippy::cast_possible_truncation, reason = "bounded by the spec")]
    let (step, jitter) = (
        node.params
            .get("phase_step")
            .and_then(Param::as_f64)
            .unwrap_or(0.0) as f32,
        node.params
            .get("phase_jitter")
            .and_then(Param::as_f64)
            .unwrap_or(0.0) as f32,
    );
    if !(-MAX_PHASE..=MAX_PHASE).contains(&step) || !(0.0..=MAX_PHASE).contains(&jitter) {
        return Err(fail(
            "param.range",
            format!(
                "phase_step must be within {MAX_PHASE} seconds and phase_jitter 0 to {MAX_PHASE}"
            ),
        ));
    }
    if (step.abs() > f32::EPSILON || jitter > f32::EPSILON) && input.instances.is_none() {
        return Err(fail(
            "anim.phase",
            "phase_step and phase_jitter offset the copies of a placed set (inst.array, inst.radial, inst.repeat); this shape is a single part",
        ));
    }
    Ok((step, jitter))
}

/// Longest clip name.
pub const MAX_CLIP_NAME: usize = 24;

/// Reads a clip name from a node's `clip` parameter: empty, or lowercase letters, digits and underscores starting
/// with a letter.
pub(crate) fn clip_name(node: &Node) -> Result<String, Fail> {
    let Some(Param::Str(text)) = node.params.get("clip") else {
        return Ok(String::new());
    };
    let text = text.trim();
    if text.is_empty() {
        return Ok(String::new());
    }
    let mut chars = text.chars();
    let valid = chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && text.len() <= MAX_CLIP_NAME;
    if valid {
        Ok(text.to_owned())
    } else {
        Err(fail_suggest(
            "anim.clip_name",
            format!(
                "clip name \"{text}\" must be lowercase letters, digits and underscores, start with a letter, and be at most {MAX_CLIP_NAME} long"
            ),
            "use lowercase letters, digits and underscores, starting with a letter",
        ))
    }
}

impl Track {
    /// Seconds from the first key to the last.
    #[must_use]
    pub fn duration(&self) -> f32 {
        self.keys.last().map_or(0.0, |key| key.time)
    }

    /// The value `seconds` into the clip: a looping track wraps past its last key, a one-shot track holds the
    /// first key before it starts and the last key after it ends.
    #[must_use]
    pub fn sample(&self, seconds: f32) -> [f32; 3] {
        let (Some(first), Some(last)) = (self.keys.first(), self.keys.last()) else {
            return [0.0; 3];
        };
        let span = last.time - first.time;
        let t = if self.looped && span > 0.0 {
            first.time + (seconds - first.time).rem_euclid(span)
        } else {
            seconds.clamp(first.time, last.time)
        };
        let next = self
            .keys
            .partition_point(|key| key.time <= t)
            .min(self.keys.len() - 1);
        let (a, b) = (self.keys[next.saturating_sub(1)], self.keys[next]);
        if (b.time - a.time).abs() < f32::EPSILON || t >= last.time {
            return if t >= last.time { last.value } else { a.value };
        }
        let fraction = (t - a.time) / (b.time - a.time);
        if self.interpolation == Interpolation::Bezier {
            let segment = next.saturating_sub(1);
            return [0, 1, 2].map(|axis| crate::bezier::sample(&self.keys, segment, t, axis));
        }
        let eased = match self.interpolation {
            Interpolation::Back => crate::easing::back_out(fraction),
            Interpolation::Bounce => crate::easing::bounce_out(fraction),
            Interpolation::Elastic => crate::easing::elastic_out(fraction),
            Interpolation::Linear | Interpolation::Bezier => fraction,
            Interpolation::Smooth => fraction * fraction * (3.0 - 2.0 * fraction),
            Interpolation::EaseIn => fraction * fraction,
            Interpolation::EaseOut => 1.0 - (1.0 - fraction) * (1.0 - fraction),
            Interpolation::Step => 0.0,
        };
        [0, 1, 2].map(|axis| a.value[axis] + (b.value[axis] - a.value[axis]) * eased)
    }
}
