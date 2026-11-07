//! `anim.wave`: a repeating oscillation (a bob, a sway, a pulse) written as one looping track.
//!
//! The wave is sampled into keys over one period, so the viewer plays it and the glTF exporter writes it with no
//! special support: it is an ordinary `anim.track`.
use std::{f32::consts::TAU, rc::Rc};

use ve_graph::{Node, Value as Param};

use crate::{
    Fail,
    anim::{
        Interpolation, MAX_TRACKS, Track, TrackProperty, clip_name, parse_keys, phases, pivot_for,
    },
    cache::Value,
    fail,
};

/// Samples per period (33 keys including the closing one).
const SAMPLES: usize = 32;

fn triple(node: &Node, name: &str, default: [f32; 3]) -> [f32; 3] {
    let Some(Param::List(items)) = node.params.get(name) else {
        return default;
    };
    let mut out = default;
    for (slot, item) in out.iter_mut().zip(items) {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "wave amounts are bounded by the spec"
        )]
        {
            *slot = item.as_f64().unwrap_or(0.0) as f32;
        }
    }
    out
}

fn number(node: &Node, name: &str, default: f32) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "wave amounts are bounded by the spec"
    )]
    let value = node
        .params
        .get(name)
        .and_then(Param::as_f64)
        .map_or(default, |v| v as f32);
    value
}

/// The wave's value in -1 to 1 at `u` periods in; both shapes start at zero and rise.
fn shape(kind: &str, u: f32) -> f32 {
    if kind == "triangle" {
        1.0 - 4.0 * ((u + 0.25).fract() - 0.5).abs()
    } else {
        (TAU * u).sin()
    }
}

/// Adds one looping oscillation track to the input.
pub(crate) fn wave(node: &Node, inputs: &[Rc<Value>]) -> Result<Value, Fail> {
    let input = inputs
        .first()
        .ok_or_else(|| fail("graph.missing_input", "anim.wave needs a shape"))?;
    let (Some(Param::Str(property)), Some(Param::Str(kind))) =
        (node.params.get("property"), node.params.get("shape"))
    else {
        return Err(fail("param.type", "anim.wave needs property and shape"));
    };
    let property = match property.as_str() {
        "position" => TrackProperty::Position,
        "rotation" => TrackProperty::Rotation,
        "scale" => TrackProperty::Scale,
        "glow" => TrackProperty::Glow,
        other => {
            return Err(fail(
                "param.range",
                format!("unknown anim.wave property \"{other}\""),
            ));
        }
    };
    let mut base = triple(node, "base", [0.0; 3]);
    // Scale and glow centre on 1; an untouched base of zero would shrink the shape away or switch it off.
    if matches!(property, TrackProperty::Scale | TrackProperty::Glow)
        && base.iter().all(|v| v.abs() < f32::EPSILON)
    {
        base = [1.0; 3];
    }
    let amplitude = triple(node, "amplitude", [0.0; 3]);
    let period = number(node, "period", 2.0);
    let phase = number(node, "phase", 0.0);
    if !(0.05..=600.0).contains(&period) {
        return Err(fail(
            "param.range",
            "anim.wave period must be 0.05 to 600 seconds",
        ));
    }
    let text: Vec<String> = (0..=SAMPLES)
        .map(|i| {
            #[expect(clippy::cast_precision_loss, reason = "i is at most 32")]
            let fraction = i as f32 / SAMPLES as f32;
            let w = shape(kind, phase + fraction);
            let v = [0, 1, 2].map(|a| base[a] + amplitude[a] * w);
            if property == TrackProperty::Glow {
                format!("{:.6}:{:.6}", period * fraction, v[0])
            } else {
                format!(
                    "{:.6}:{:.6},{:.6},{:.6}",
                    period * fraction,
                    v[0],
                    v[1],
                    v[2]
                )
            }
        })
        .collect();
    let keys = parse_keys(&text.join(";"), property)?;
    let mut value = (**input).clone();
    if value.animation.len() >= MAX_TRACKS {
        return Err(fail(
            "param.range",
            format!("a part can carry at most {MAX_TRACKS} tracks"),
        ));
    }
    let (phase_step, phase_jitter) = phases(node, input)?;
    value.animation.push(Track {
        property,
        keys,
        interpolation: Interpolation::Linear,
        looped: true,
        pivot: pivot_for(node, input)?,
        clip: clip_name(node)?,
        phase_step,
        phase_jitter,
    });
    Ok(value)
}
