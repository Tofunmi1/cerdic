//! The runtime: named inputs in, a blended pose out.

use std::collections::{BTreeMap, HashMap};

use crate::blend::{ClipWeight, Pose, blend, consume, length, locate, merge, smoothstep, wrap};
use crate::graph::{Condition, MAX_LAYERS, Motion, StateMachine};
use crate::params::Params;

/// Longest time step applied in one call, in seconds. A larger `dt` (a stalled frame, or a test)
/// is clamped to this, so one call cannot skip a whole clip; `1.0` is well inside the clamp.
pub const MAX_STEP: f32 = 10.0;

/// A running state machine.
///
/// Validate the graph with [`StateMachine::validate`] first: `new` trusts it and falls back to the
/// first state if the entry name is missing.
#[derive(Clone, Debug)]
pub struct Machine {
    graph: StateMachine,
    lengths: HashMap<String, f32>,
    params: Params,
    /// The state transitions are evaluated from (the last target).
    current: usize,
    /// Local clip time in seconds for every active state.
    times: BTreeMap<usize, f32>,
    /// Current weight of every active state; sums to one.
    weights: BTreeMap<usize, f32>,
    fade: Option<Fade>,
    /// The overlay layers' clocks and weights.
    extras_runtime: crate::machine_extras::Runtime,
}

/// An in-progress crossfade from a snapshot of the weights to `to`.
#[derive(Clone, Debug)]
struct Fade {
    /// Weights when the fade began.
    from: BTreeMap<usize, f32>,
    /// State being faded in.
    to: usize,
    /// Seconds since the fade began.
    elapsed: f32,
    /// Total fade length in seconds.
    duration: f32,
}

impl Machine {
    /// Starts the machine in the graph's entry state.
    ///
    /// `clip_lengths` gives each clip's length in seconds, used to wrap looping clips and clamp
    /// one-shots; a clip that is missing or has a non-positive length is left unwrapped.
    #[must_use]
    pub fn new(graph: StateMachine, clip_lengths: &HashMap<String, f32>) -> Self {
        let current = graph
            .states
            .iter()
            .position(|state| state.name == graph.entry)
            .unwrap_or(0);
        let mut times = BTreeMap::new();
        let mut weights = BTreeMap::new();
        if !graph.states.is_empty() {
            times.insert(current, 0.0);
            weights.insert(current, 1.0);
        }
        Self {
            extras_runtime: crate::machine_extras::Runtime::new(&graph.extras),
            graph,
            lengths: clip_lengths.clone(),
            params: Params::new(),
            current,
            times,
            weights,
            fade: None,
        }
    }

    /// Sets a float parameter (for example `speed`).
    pub fn set_float(&mut self, name: &str, value: f32) {
        self.params.set_float(name, value);
    }

    /// Sets a bool parameter (for example `grounded`).
    pub fn set_bool(&mut self, name: &str, value: bool) {
        self.params.set_bool(name, value);
    }

    /// Raises a one-shot trigger; the transition that reads it lowers it.
    pub fn trigger(&mut self, name: &str) {
        self.params.trigger(name);
    }

    /// The current inputs.
    #[must_use]
    pub fn params(&self) -> &Params {
        &self.params
    }

    /// Name of the state transitions are evaluated from, if the graph has any.
    #[must_use]
    pub fn current_state(&self) -> Option<&str> {
        self.graph
            .states
            .get(self.current)
            .map(|state| state.name.as_str())
    }

    /// Advances by `dt` seconds and returns the pose to play, with weights summing to one.
    ///
    /// `dt` is clamped to [`MAX_STEP`]. At most one transition fires per call; a transition that
    /// fires starts its crossfade at the current weights, so an interruption never pops.
    pub fn step(&mut self, dt: f32) -> Pose {
        let dt = if dt.is_finite() {
            dt.clamp(0.0, MAX_STEP)
        } else {
            0.0
        };
        let before = self.times.clone();
        self.advance(dt);
        if let Some(index) = self.select() {
            self.fire(index);
        }
        let mut pose = self.pose();
        pose.events = crate::machine_extras::events(&self.graph, &self.lengths, &before, &self.times, &self.weights, dt);
        self.extras_runtime
            .advance(&self.graph.extras, &self.params, &self.lengths, dt);
        pose.overlays = self.extras_runtime.overlays(&self.graph.extras);
        pose
    }

    /// Advances every active clip clock and the running crossfade.
    fn advance(&mut self, dt: f32) {
        for (&index, time) in &mut self.times {
            let Some(state) = self.graph.states.get(index) else {
                continue;
            };
            match &state.motion {
                Motion::Clip {
                    clip,
                    speed,
                    looping,
                } => {
                    *time += dt * speed;
                    if let Some(length) = length(&self.lengths, clip) {
                        *time = if *looping {
                            wrap(*time, length)
                        } else {
                            time.clamp(0.0, length)
                        };
                    }
                }
                Motion::Blend1D { .. } => *time += dt,
            }
        }
        let Some(fade) = self.fade.take() else {
            return;
        };
        let elapsed = fade.elapsed + dt;
        if elapsed >= fade.duration {
            self.commit(fade.to);
        } else {
            let p = if fade.duration > 0.0 {
                smoothstep(elapsed / fade.duration)
            } else {
                1.0
            };
            self.weights = blend(&fade.from, fade.to, p);
            self.fade = Some(Fade { elapsed, ..fade });
        }
    }

    /// Gives `to` the whole weight and forgets the other states.
    fn commit(&mut self, to: usize) {
        self.weights.clear();
        self.weights.insert(to, 1.0);
        self.times.retain(|index, _| *index == to);
        self.fade = None;
    }

    /// The first transition whose source and conditions match.
    fn select(&self) -> Option<usize> {
        for (index, transition) in self.graph.transitions.iter().enumerate() {
            if let Some(from) = &transition.from {
                let Some(source) = self.state_index(from) else {
                    continue;
                };
                if source != self.current {
                    continue;
                }
            }
            if !transition
                .conditions
                .iter()
                .all(|condition| self.holds(condition))
            {
                continue;
            }
            if let Some(exit) = transition.exit_time
                && self.fraction(self.current) < exit
            {
                continue;
            }
            let Some(target) = self.state_index(&transition.to) else {
                continue;
            };
            // Already here (or already fading here): firing again would restart the fade forever.
            if target == self.current && self.fade.as_ref().is_none_or(|fade| fade.to == target) {
                continue;
            }
            return Some(index);
        }
        None
    }

    /// Fires `index`: consumes its triggers, changes state and starts its crossfade.
    fn fire(&mut self, index: usize) {
        let Some(transition) = self.graph.transitions.get(index).cloned() else {
            return;
        };
        let Some(target) = self.state_index(&transition.to) else {
            return;
        };
        for condition in &transition.conditions {
            consume(&mut self.params, condition);
        }
        self.current = target;
        self.times.entry(target).or_insert(0.0);
        if transition.duration > 0.0 {
            self.fade = Some(Fade {
                from: self.weights.clone(),
                to: target,
                elapsed: 0.0,
                duration: transition.duration,
            });
        } else {
            self.commit(target);
        }
    }

    /// Whether `condition` holds for the current parameters.
    fn holds(&self, condition: &Condition) -> bool {
        match condition {
            Condition::Greater { parameter, value } => {
                self.params.float(parameter).is_some_and(|v| v > *value)
            }
            Condition::Less { parameter, value } => {
                self.params.float(parameter).is_some_and(|v| v < *value)
            }
            Condition::Equals { parameter, value } => {
                self.params.boolean(parameter) == Some(*value)
            }
            Condition::Trigger { parameter } => self.params.triggered(parameter),
            Condition::And(list) => list.iter().all(|inner| self.holds(inner)),
        }
    }

    /// How far the state's clip has played, `0.0` to `1.0`; `1.0` when its length is unknown.
    fn fraction(&self, index: usize) -> f32 {
        let Some(state) = self.graph.states.get(index) else {
            return 1.0;
        };
        let Some(&time) = self.times.get(&index) else {
            return 1.0;
        };
        match &state.motion {
            Motion::Clip { clip, .. } => match length(&self.lengths, clip) {
                Some(length) => (time / length).clamp(0.0, 1.0),
                None => 1.0,
            },
            Motion::Blend1D { .. } => 1.0,
        }
    }

    /// Builds the blended pose from the current weights.
    fn pose(&self) -> Pose {
        let mut clips: BTreeMap<String, (f32, f32)> = BTreeMap::new();
        for (&index, &weight) in &self.weights {
            if weight <= 0.0 {
                continue;
            }
            let Some(state) = self.graph.states.get(index) else {
                continue;
            };
            match &state.motion {
                Motion::Clip { clip, looping, .. } => {
                    let time = self.clip_time(index, clip, *looping);
                    merge(&mut clips, clip, weight, time);
                }
                Motion::Blend1D { parameter, points } => {
                    let value = self
                        .params
                        .float(parameter)
                        .unwrap_or_else(|| points.first().map_or(0.0, |point| point.value));
                    let (low, high, t) = locate(points, value);
                    if let Some(point) = points.get(low) {
                        let time = self.clip_time(index, &point.clip, true);
                        merge(&mut clips, &point.clip, weight * (1.0 - t), time);
                    }
                    if high != low
                        && let Some(point) = points.get(high)
                    {
                        let time = self.clip_time(index, &point.clip, true);
                        merge(&mut clips, &point.clip, weight * t, time);
                    }
                }
            }
        }
        let mut layers: Vec<ClipWeight> = clips
            .into_iter()
            .map(|(clip, (weight, time))| ClipWeight { clip, time, weight })
            .collect();
        layers.sort_by(|a, b| {
            b.weight
                .total_cmp(&a.weight)
                .then_with(|| a.clip.cmp(&b.clip))
        });
        layers.truncate(MAX_LAYERS);
        let total: f32 = layers.iter().map(|layer| layer.weight).sum();
        if total > 0.0 {
            for layer in &mut layers {
                layer.weight /= total;
            }
        }
        Pose {
            layers,
            ..Pose::default()
        }
    }

    /// The state's clip time for output, wrapped or clamped when its length is known.
    fn clip_time(&self, index: usize, clip: &str, looping: bool) -> f32 {
        let time = self.times.get(&index).copied().unwrap_or(0.0);
        match length(&self.lengths, clip) {
            Some(length) => {
                if looping {
                    wrap(time, length)
                } else {
                    time.clamp(0.0, length)
                }
            }
            None => time,
        }
    }

    /// The index of the state named `name`.
    fn state_index(&self, name: &str) -> Option<usize> {
        self.graph
            .states
            .iter()
            .position(|state| state.name == name)
    }
}
