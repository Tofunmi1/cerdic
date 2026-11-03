//! What a running machine does for its layers and events.
use std::collections::{BTreeMap, HashMap};

use crate::blend::{length, wrap};
use crate::events::crossed;
use crate::graph::{Motion, StateMachine};
use crate::layers::{EVENT_WEIGHT, Extras, FiredEvent, LayerWeight, OverlayWeight};
use crate::params::Params;

/// The clocks and smoothed weights of the machine's overlay layers.
#[derive(Clone, Debug, Default)]
pub(crate) struct Runtime {
    times: Vec<f32>,
    weights: Vec<f32>,
}

impl Runtime {
    /// A fixed-weight layer starts at its weight; a parameter-driven one starts off.
    pub(crate) fn new(extras: &Extras) -> Self {
        Self {
            times: vec![0.0; extras.layers.len()],
            weights: extras
                .layers
                .iter()
                .map(|layer| match layer.weight {
                    LayerWeight::Fixed(w) => w,
                    LayerWeight::Parameter(_) => 0.0,
                })
                .collect(),
        }
    }

    /// Moves every layer's weight towards its target (linearly, in `fade` seconds from 0 to 1) and runs the clocks of the layers that are on.
    pub(crate) fn advance(&mut self, extras: &Extras, params: &Params, lengths: &HashMap<String, f32>, dt: f32) {
        for (index, layer) in extras.layers.iter().enumerate() {
            let target = match &layer.weight {
                LayerWeight::Fixed(w) => *w,
                LayerWeight::Parameter(name) => params.float(name).map_or(0.0, |v| if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 }),
            };
            let weight = &mut self.weights[index];
            *weight = if layer.fade > 0.0 {
                let step = dt / layer.fade;
                if target > *weight { (*weight + step).min(target) } else { (*weight - step).max(target) }
            } else {
                target
            };
            let time = &mut self.times[index];
            if *weight <= 0.0 && target <= 0.0 {
                // Off: the clip starts from its first frame next time it is wanted.
                *time = 0.0;
                continue;
            }
            *time += dt * layer.speed;
            if let Some(len) = length(lengths, &layer.clip) {
                *time = if layer.looping { wrap(*time, len) } else { time.clamp(0.0, len) };
            }
        }
    }

    /// The layers that have a weight this step.
    pub(crate) fn overlays(&self, extras: &Extras) -> Vec<OverlayWeight> {
        extras
            .layers
            .iter()
            .enumerate()
            .filter(|(index, _)| self.weights[*index] > 0.0)
            .map(|(index, layer)| OverlayWeight {
                layer: layer.name.clone(),
                clip: layer.clip.clone(),
                time: self.times[index],
                weight: self.weights[index],
                additive: layer.additive,
                mask: layer.mask.clone(),
            })
            .collect()
    }
}

/// The events of one clip passed as its time went from `from` to `to` (raw, unwrapped seconds), each once.
fn between(graph: &StateMachine, clip: &str, len: f32, from: f32, to: f32, looping: bool, fresh: bool) -> Vec<FiredEvent> {
    let events: Vec<&crate::layers::ClipEvent> = graph.extras.events.iter().filter(|e| e.clip == clip).collect();
    if events.is_empty() {
        return Vec::new();
    }
    let times: Vec<f32> = events.iter().map(|e| e.time).collect();
    let fired = |indices: Vec<usize>| -> Vec<FiredEvent> {
        indices
            .into_iter()
            .map(|i| FiredEvent {
                name: events[i].name.clone(),
                clip: clip.to_owned(),
            })
            .collect()
    };
    if !looping {
        let (from, to) = (if fresh { -1.0 } else { from.clamp(0.0, len) }, to.min(len));
        return fired(crossed(&times, len, from, to, false));
    }
    let start = if fresh { -1.0 } else { wrap(from, len) };
    let wraps = (to / len).floor() - if fresh { 0.0 } else { (from.max(0.0) / len).floor() };
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to a small non-negative count"
    )]
    let wraps = wraps.clamp(0.0, 8.0) as u32;
    let end = wrap(to, len);
    if wraps == 0 {
        return fired(crossed(&times, len, start, end, false));
    }
    let mut out = fired(crossed(&times, len, start, end, true));
    // Whole loops passed in one long step: every event once per extra loop.
    let all = crossed(&times, len, -1.0, len, false);
    for _ in 1..wraps {
        out.extend(fired(all.clone()));
    }
    out
}

/// The events fired this step by every state that plays with enough weight. `before` and `after` are the states' clip times around the step.
pub(crate) fn events(
    graph: &StateMachine,
    lengths: &HashMap<String, f32>,
    before: &BTreeMap<usize, f32>,
    after: &BTreeMap<usize, f32>,
    weights: &BTreeMap<usize, f32>,
    dt: f32,
) -> Vec<FiredEvent> {
    let mut out = Vec::new();
    if graph.extras.events.is_empty() {
        return out;
    }
    for (&index, &weight) in weights {
        if weight <= EVENT_WEIGHT {
            continue;
        }
        let (Some(state), Some(&now)) = (graph.states.get(index), after.get(&index)) else {
            continue;
        };
        let (fresh, was) = match before.get(&index) {
            Some(&t) => (false, t),
            None => (true, 0.0),
        };
        match &state.motion {
            Motion::Clip { clip, speed, looping } => {
                let Some(len) = length(lengths, clip) else {
                    continue;
                };
                let to = if fresh { dt * speed } else { was + dt * speed };
                out.extend(between(graph, clip, len, was, to, *looping, fresh));
            }
            Motion::Blend1D { points, .. } => {
                for point in points {
                    let Some(len) = length(lengths, &point.clip) else {
                        continue;
                    };
                    out.extend(between(graph, &point.clip, len, was, now, true, fresh));
                }
            }
        }
    }
    out
}
