use super::utils::finite_clamp;
use super::{Fact, MemoryStore};

impl MemoryStore {
    /// Sets or replaces a fact and returns the previous value.
    pub fn set_fact(
        &mut self,
        key: impl Into<String>,
        value: impl Into<String>,
        confidence: f32,
        now: f64,
    ) -> Option<Fact> {
        self.facts.insert(
            key.into(),
            Fact {
                value: value.into(),
                confidence: finite_clamp(confidence, 0.0, 1.0),
                time: now,
            },
        )
    }
    /// Gets a fact by key.
    #[must_use]
    pub fn fact(&self, key: &str) -> Option<&Fact> {
        self.facts.get(key)
    }
}
