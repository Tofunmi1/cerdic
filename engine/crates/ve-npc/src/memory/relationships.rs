use super::utils::finite_clamp;
use super::{MemoryStore, Relationship};

impl MemoryStore {
    /// Gets a relationship, defaulting to zero values.
    #[must_use]
    pub fn relationship(&self, other: &str) -> Relationship {
        self.relationships.get(other).copied().unwrap_or_default()
    }
    /// Adjusts and clamps a relationship.
    pub fn adjust_relationship(&mut self, other: impl Into<String>, da: f32, dt: f32, df: f32) {
        let r = self.relationships.entry(other.into()).or_default();
        r.affinity = finite_clamp(r.affinity + da, -1.0, 1.0);
        r.trust = finite_clamp(r.trust + dt, -1.0, 1.0);
        r.familiarity = finite_clamp(r.familiarity + df, 0.0, 1.0);
    }
}
