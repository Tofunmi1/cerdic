//! Pose layers and the small arithmetic the runtime blends with.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::graph::{BlendPoint, Condition};
use crate::params::Params;

/// One clip in a pose.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClipWeight {
    /// Clip name.
    pub clip: String,
    /// Playback time in seconds: wrapped for a looping clip, clamped for a one-shot.
    pub time: f32,
    /// Blend weight in `0.0..=1.0`; the weights in a pose sum to one.
    pub weight: f32,
}

/// The clips to play this step, blended. Layers are heaviest first and their weights sum to one,
/// unless the machine has no state at all.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Pose {
    /// One entry per clip.
    pub layers: Vec<ClipWeight>,
    /// Overlay layers with a weight this step, in the machine's order (a wave over the walk).
    #[serde(default)]
    pub overlays: Vec<crate::layers::OverlayWeight>,
    /// Clip events that happened during this step.
    #[serde(default)]
    pub events: Vec<crate::layers::FiredEvent>,
}

/// A clip's length, when it was given and is usable.
pub(crate) fn length(lengths: &HashMap<String, f32>, clip: &str) -> Option<f32> {
    lengths
        .get(clip)
        .copied()
        .filter(|length| length.is_finite() && *length > 0.0)
}

/// Lowers every trigger a firing transition read.
pub(crate) fn consume(params: &mut Params, condition: &Condition) {
    match condition {
        Condition::Trigger { parameter } => params.clear_trigger(parameter),
        Condition::And(list) => {
            for inner in list {
                consume(params, inner);
            }
        }
        _ => {}
    }
}

/// A convex blend of `from` and a single target weight `p`.
pub(crate) fn blend(from: &BTreeMap<usize, f32>, to: usize, p: f32) -> BTreeMap<usize, f32> {
    let mut out = BTreeMap::new();
    for (&index, &weight) in from {
        let weight = weight * (1.0 - p);
        if weight > 0.0 {
            out.insert(index, weight);
        }
    }
    let existing = out.get(&to).copied().unwrap_or(0.0);
    out.insert(to, existing + p);
    out
}

/// Adds `weight` to `clip`, keeping the time of whichever contributor is heavier.
pub(crate) fn merge(clips: &mut BTreeMap<String, (f32, f32)>, clip: &str, weight: f32, time: f32) {
    if weight <= 0.0 {
        return;
    }
    match clips.get_mut(clip) {
        Some(entry) => {
            if weight > entry.0 {
                entry.1 = time;
            }
            entry.0 += weight;
        }
        None => {
            clips.insert(clip.to_owned(), (weight, time));
        }
    }
}

/// The two points around `value` and the blend fraction between them (clamped outside the range).
pub(crate) fn locate(points: &[BlendPoint], value: f32) -> (usize, usize, f32) {
    if points.is_empty() {
        return (0, 0, 0.0);
    }
    let last = points.len() - 1;
    if value <= points[0].value {
        return (0, 0, 0.0);
    }
    if value >= points[last].value {
        return (last, last, 0.0);
    }
    for index in 0..last {
        let (low, high) = (&points[index], &points[index + 1]);
        if value >= low.value && value <= high.value {
            let span = high.value - low.value;
            let t = if span > 0.0 {
                (value - low.value) / span
            } else {
                0.0
            };
            return (index, index + 1, t);
        }
    }
    (last, last, 0.0)
}

/// Wraps `time` into `0.0..length`.
pub(crate) fn wrap(time: f32, length: f32) -> f32 {
    let wrapped = time % length;
    if wrapped < 0.0 {
        wrapped + length
    } else {
        wrapped
    }
}

/// `p * p * (3 - 2p)`, an arithmetic-only ease.
pub(crate) fn smoothstep(p: f32) -> f32 {
    let p = p.clamp(0.0, 1.0);
    p * p * (3.0 - 2.0 * p)
}
