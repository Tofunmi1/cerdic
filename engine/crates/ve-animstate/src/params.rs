//! The named inputs that drive transitions.

use std::collections::{BTreeMap, BTreeSet};

/// Named inputs: floats, bools and one-shot triggers.
///
/// A parameter that was never set counts as absent, so a condition on it is false. A non-finite
/// float is ignored on the way in, so a bad input cannot poison the simulation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Params {
    floats: BTreeMap<String, f32>,
    bools: BTreeMap<String, bool>,
    triggers: BTreeSet<String>,
}

impl Params {
    /// An empty set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the float `name` to `value`; a non-finite value is ignored.
    pub fn set_float(&mut self, name: &str, value: f32) {
        if value.is_finite() {
            self.floats.insert(name.to_owned(), value);
        }
    }

    /// The current value of the float `name`, if it was set.
    #[must_use]
    pub fn float(&self, name: &str) -> Option<f32> {
        self.floats.get(name).copied()
    }

    /// Sets the bool `name` to `value`.
    pub fn set_bool(&mut self, name: &str, value: bool) {
        self.bools.insert(name.to_owned(), value);
    }

    /// The current value of the bool `name`, if it was set.
    #[must_use]
    pub fn boolean(&self, name: &str) -> Option<bool> {
        self.bools.get(name).copied()
    }

    /// Raises the one-shot `name`; it is lowered when a transition consumes it.
    pub fn trigger(&mut self, name: &str) {
        self.triggers.insert(name.to_owned());
    }

    /// Whether the one-shot `name` is raised.
    #[must_use]
    pub fn triggered(&self, name: &str) -> bool {
        self.triggers.contains(name)
    }

    /// Lowers the one-shot `name`.
    pub(crate) fn clear_trigger(&mut self, name: &str) {
        self.triggers.remove(name);
    }
}
