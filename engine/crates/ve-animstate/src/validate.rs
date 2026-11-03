//! Validation of a [`StateMachine`](crate::graph::StateMachine).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::graph::{
    Condition, Error, MAX_BLEND_POINTS, MAX_STATES, MAX_TRANSITIONS, Motion, State, StateMachine,
    Transition,
};

impl StateMachine {
    /// Checks names, references, numbers, bounds and the zero-duration cycle rule.
    ///
    /// # Errors
    /// Returns [`Error`], naming the state, transition or parameter to change.
    pub fn validate(&self) -> Result<(), Error> {
        if self.states.len() > MAX_STATES {
            return Err(Error::Machine {
                reason: format!(
                    "{} states is more than the {MAX_STATES} allowed; split the machine",
                    self.states.len()
                ),
            });
        }
        if self.transitions.len() > MAX_TRANSITIONS {
            return Err(Error::Machine {
                reason: format!(
                    "{} transitions is more than the {MAX_TRANSITIONS} allowed; split the machine",
                    self.transitions.len()
                ),
            });
        }
        let mut by_name: BTreeMap<&str, usize> = BTreeMap::new();
        for (index, state) in self.states.iter().enumerate() {
            if state.name.is_empty() {
                return Err(Error::State {
                    state: state.name.clone(),
                    reason: "the state name is empty; give it a name".to_owned(),
                });
            }
            if by_name.insert(state.name.as_str(), index).is_some() {
                return Err(Error::State {
                    state: state.name.clone(),
                    reason: "duplicate state name; rename one of the two states".to_owned(),
                });
            }
        }
        for state in &self.states {
            check_motion(state)?;
        }
        if !by_name.contains_key(self.entry.as_str()) {
            return Err(Error::Machine {
                reason: format!(
                    "entry {:?} is not a state; add that state or set `entry` to an existing one",
                    self.entry
                ),
            });
        }
        for (index, transition) in self.transitions.iter().enumerate() {
            self.check_transition(index, transition, &by_name)?;
        }
        self.check_parameters()?;
        self.check_cycles(&by_name)?;
        self.extras.validate()?;
        Ok(())
    }

    /// A transition names states that exist and carries finite, in-range numbers.
    fn check_transition(
        &self,
        index: usize,
        transition: &Transition,
        by_name: &BTreeMap<&str, usize>,
    ) -> Result<(), Error> {
        let fail = |reason: String| Error::Transition { index, reason };
        if let Some(from) = &transition.from
            && !by_name.contains_key(from.as_str())
        {
            return Err(fail(format!(
                "source {from:?} is not a state; fix the name or add the state"
            )));
        }
        if !by_name.contains_key(transition.to.as_str()) {
            return Err(fail(format!(
                "target {:?} is not a state; fix the name or add the state",
                transition.to
            )));
        }
        if !transition.duration.is_finite() || transition.duration < 0.0 {
            return Err(fail(
                "crossfade duration must be a finite, non-negative number of seconds".to_owned(),
            ));
        }
        if let Some(exit) = transition.exit_time {
            if !exit.is_finite() || !(0.0..=1.0).contains(&exit) {
                return Err(fail("exit_time must be a fraction from 0 to 1".to_owned()));
            }
            let Some(&source) = transition
                .from
                .as_ref()
                .and_then(|from| by_name.get(from.as_str()))
            else {
                return Err(fail(
                    "exit_time needs a source state; set `from` to a one-shot clip state"
                        .to_owned(),
                ));
            };
            if !matches!(
                self.states.get(source).map(|state| &state.motion),
                Some(Motion::Clip { looping: false, .. })
            ) {
                return Err(fail(
                    "exit_time needs the source state to be a one-shot clip (looping = false)"
                        .to_owned(),
                ));
            }
        }
        Ok(())
    }

    /// Every parameter name is used as exactly one kind.
    fn check_parameters(&self) -> Result<(), Error> {
        let mut kinds: BTreeMap<String, Kind> = BTreeMap::new();
        for state in &self.states {
            if let Motion::Blend1D { parameter, .. } = &state.motion {
                declare(&mut kinds, parameter, Kind::Float)?;
            }
        }
        for transition in &self.transitions {
            for condition in &transition.conditions {
                declare_condition(&mut kinds, condition)?;
            }
        }
        Ok(())
    }

    /// No cycle of unconditional zero-duration transitions would never settle.
    fn check_cycles(&self, by_name: &BTreeMap<&str, usize>) -> Result<(), Error> {
        let reachable = self.reachable(by_name);
        let mut indegree: BTreeMap<usize, usize> =
            reachable.iter().map(|&index| (index, 0)).collect();
        let mut edges: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for transition in &self.transitions {
            if transition.duration > 0.0 || !transition.conditions.is_empty() {
                continue;
            }
            let Some(&target) = by_name.get(transition.to.as_str()) else {
                continue;
            };
            let sources: Vec<usize> = match &transition.from {
                Some(from) => by_name.get(from.as_str()).copied().into_iter().collect(),
                None => reachable.iter().copied().collect(),
            };
            for source in sources {
                if !reachable.contains(&source) || !reachable.contains(&target) {
                    continue;
                }
                edges.entry(source).or_default().push(target);
                *indegree.entry(target).or_insert(0) += 1;
            }
        }
        let mut queue: VecDeque<usize> = indegree
            .iter()
            .filter(|(_, degree)| **degree == 0)
            .map(|(&index, _)| index)
            .collect();
        while let Some(node) = queue.pop_front() {
            for &target in edges.get(&node).into_iter().flatten() {
                if let Some(degree) = indegree.get_mut(&target) {
                    *degree = degree.saturating_sub(1);
                    if *degree == 0 {
                        queue.push_back(target);
                    }
                }
            }
        }
        let stuck: Vec<&str> = indegree
            .iter()
            .filter(|(_, degree)| **degree > 0)
            .filter_map(|(&index, _)| self.states.get(index).map(|state| state.name.as_str()))
            .collect();
        if stuck.is_empty() {
            Ok(())
        } else {
            Err(Error::Machine {
                reason: format!(
                    "unconditional zero-duration transitions form a cycle through {}; give one of them a duration or a condition",
                    stuck.join(", ")
                ),
            })
        }
    }

    /// States reachable from the entry through any transition.
    fn reachable(&self, by_name: &BTreeMap<&str, usize>) -> BTreeSet<usize> {
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::new();
        if let Some(entry) = by_name.get(self.entry.as_str()).copied() {
            seen.insert(entry);
            queue.push_back(entry);
        }
        while let Some(state) = queue.pop_front() {
            for transition in &self.transitions {
                let matches = match &transition.from {
                    Some(from) => by_name.get(from.as_str()).copied() == Some(state),
                    None => true,
                };
                if matches
                    && let Some(target) = by_name.get(transition.to.as_str()).copied()
                    && seen.insert(target)
                {
                    queue.push_back(target);
                }
            }
        }
        seen
    }
}

/// A state's motion is numerically usable.
fn check_motion(state: &State) -> Result<(), Error> {
    let fail = |reason: String| Error::State {
        state: state.name.clone(),
        reason,
    };
    match &state.motion {
        Motion::Clip { speed, .. } => {
            if !speed.is_finite() {
                return Err(fail("clip speed must be a finite number".to_owned()));
            }
        }
        Motion::Blend1D { points, .. } => {
            if points.is_empty() {
                return Err(fail("the blend has no points; add at least one".to_owned()));
            }
            if points.len() > MAX_BLEND_POINTS {
                return Err(fail(format!(
                    "the blend has {} points, more than the {MAX_BLEND_POINTS} allowed; remove some",
                    points.len()
                )));
            }
            for (index, point) in points.iter().enumerate() {
                if !point.value.is_finite() {
                    return Err(fail(format!("blend point {index} value must be finite")));
                }
                if index > 0 && point.value <= points[index - 1].value {
                    return Err(fail(format!(
                        "blend point {index} value {} is not greater than the one before it; sort the points by value",
                        point.value
                    )));
                }
            }
        }
    }
    Ok(())
}

/// The kind of a parameter, which two uses must agree on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// Sampled by `Greater`, `Less` or a blend space.
    Float,
    /// Tested by `Equals`.
    Bool,
    /// Consumed by `Trigger`.
    Trigger,
}

impl Kind {
    /// The word used in error messages.
    fn word(self) -> &'static str {
        match self {
            Self::Float => "float",
            Self::Bool => "bool",
            Self::Trigger => "trigger",
        }
    }
}

/// Records that `name` is used as `kind`, refusing a different earlier use.
fn declare(kinds: &mut BTreeMap<String, Kind>, name: &str, kind: Kind) -> Result<(), Error> {
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(Error::Parameter {
            parameter: name.to_owned(),
            reason: "names must be non-empty ASCII letters, digits or underscores".to_owned(),
        });
    }
    match kinds.get(name) {
        Some(existing) if *existing != kind => Err(Error::Parameter {
            parameter: name.to_owned(),
            reason: format!(
                "used as a {} and as a {}; give one of them a different name",
                existing.word(),
                kind.word()
            ),
        }),
        _ => {
            kinds.insert(name.to_owned(), kind);
            Ok(())
        }
    }
}

/// Declares every parameter a condition names.
fn declare_condition(
    kinds: &mut BTreeMap<String, Kind>,
    condition: &Condition,
) -> Result<(), Error> {
    match condition {
        Condition::Greater { parameter, .. } | Condition::Less { parameter, .. } => {
            declare(kinds, parameter, Kind::Float)
        }
        Condition::Equals { parameter, .. } => declare(kinds, parameter, Kind::Bool),
        Condition::Trigger { parameter } => declare(kinds, parameter, Kind::Trigger),
        Condition::And(list) => {
            for inner in list {
                declare_condition(kinds, inner)?;
            }
            Ok(())
        }
    }
}
