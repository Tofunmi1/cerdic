//! `anim.simplify`: fewer keys for the same motion. A clip made by a script or sampled densely has a key at every few milliseconds; most of them lie on
//! what the interpolation between their neighbours would give anyway. Keys are dropped, least needed first, while the simplified track stays within a
//! tolerance of the original everywhere (checked at every original key and between them). The tolerance is a distance at the TIP of the bone: for a
//! rotation track on a bone the angle allowed is that distance over the bone's length, so a long bone keeps more keys than a short one, and a
//! position track is held to the distance directly.

use std::rc::Rc;

use ve_graph::{Node, Value as Param};

use crate::anim::{Interpolation, Key, Track, TrackProperty};
use crate::cache::Value;
use crate::{Fail, fail};

pub(crate) fn run(node: &Node, inputs: &[Rc<Value>]) -> Option<Result<Value, Fail>> {
    (node.op.family().as_str() == "anim.simplify").then(|| simplify(node, inputs))
}

/// The largest difference between two tracks over the clip, sampled at `samples_per_span` points inside each original key span.
fn worst_error(original: &Track, candidate: &Track, samples_per_span: usize) -> (f32, f32) {
    let mut worst = (0.0_f32, 0.0_f32);
    for pair in original.keys.windows(2) {
        for s in 0..samples_per_span {
            let t =
                pair[0].time + (pair[1].time - pair[0].time) * s as f32 / samples_per_span as f32;
            let (a, b) = (original.sample(t), candidate.sample(t));
            let e = (0..3).map(|i| (a[i] - b[i]).abs()).fold(0.0, f32::max);
            if e > worst.0 {
                worst = (e, t);
            }
        }
    }
    let last = original.keys.last().map_or(0.0, |k| k.time);
    let (a, b) = (original.sample(last), candidate.sample(last));
    let e = (0..3).map(|i| (a[i] - b[i]).abs()).fold(0.0, f32::max);
    if e > worst.0 {
        worst = (e, last);
    }
    worst
}

/// Keeps the endpoints and adds the original key nearest the worst error until the track is within `tolerance` (in the track's own units).
#[must_use]
pub(crate) fn decimate(track: &Track, tolerance: f32) -> Track {
    let n = track.keys.len();
    if n <= 2 || matches!(track.interpolation, Interpolation::Step) {
        return track.clone();
    }
    let mut kept = vec![0, n - 1];
    loop {
        let keys: Vec<Key> = kept.iter().map(|&i| track.keys[i]).collect();
        let candidate = Track {
            keys,
            ..track.clone()
        };
        let (error, at) = worst_error(track, &candidate, 4);
        if error <= tolerance || kept.len() == n {
            return candidate;
        }
        // The not-yet-kept original key closest in time to the worst point.
        let Some(best) = (0..n).filter(|i| !kept.contains(i)).min_by(|&a, &b| {
            (track.keys[a].time - at)
                .abs()
                .total_cmp(&(track.keys[b].time - at).abs())
        }) else {
            return candidate;
        };
        kept.push(best);
        kept.sort_unstable();
    }
}

fn simplify(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = inputs.first().ok_or_else(|| {
        fail(
            "graph.missing_input",
            "anim.simplify needs a part with tracks",
        )
    })?;
    let tolerance = node
        .params
        .get("tolerance")
        .and_then(Param::as_f64)
        .unwrap_or(0.002) as f32;
    let max_angle = node
        .params
        .get("max_angle")
        .and_then(Param::as_f64)
        .unwrap_or(1.0) as f32;
    let reach = input
        .bone
        .map(|b| {
            ((b.tail[0] - b.head[0]).powi(2)
                + (b.tail[1] - b.head[1]).powi(2)
                + (b.tail[2] - b.head[2]).powi(2))
            .sqrt()
        })
        .filter(|l| *l > 1.0e-4)
        .unwrap_or(1.0);
    let mut value = (**input).clone();
    for track in &mut value.animation {
        let allowed = match track.property {
            TrackProperty::Position => tolerance,
            // The angle that moves the tip by `tolerance`, capped so a very short bone still keeps its shape.
            TrackProperty::Rotation => (tolerance / reach).to_degrees().min(max_angle),
            TrackProperty::Scale => tolerance,
            // On/off and brightness tracks are not worth thinning.
            _ => continue,
        };
        *track = decimate(track, allowed);
    }
    Ok(value)
}
