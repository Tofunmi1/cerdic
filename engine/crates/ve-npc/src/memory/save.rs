use super::{MemoryError, MemoryKind, MemoryStore};

impl MemoryStore {
    /// Sum of importance added since the last reflection, excluding reflection memories.
    #[must_use]
    pub fn importance_since_reflection(&self) -> f32 {
        self.memories
            .iter()
            .filter(|m| m.time > self.reflected_at && m.kind != MemoryKind::Reflection)
            .map(|m| m.importance)
            .sum()
    }
    /// Whether accumulated importance reaches a threshold.
    #[must_use]
    pub fn reflection_due(&self, threshold: f32) -> bool {
        self.importance_since_reflection() >= threshold
    }
    /// Marks the reflection boundary at `now`.
    pub fn mark_reflected(&mut self, now: f64) {
        self.reflected_at = now;
    }
    /// Serializes the store as stable JSON.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
    /// Loads a store, preserving ids and all saved state.
    ///
    /// # Errors
    /// Returns [`MemoryError::BadSave`] when `json` is invalid or incompatible.
    pub fn from_json(json: &str) -> Result<Self, MemoryError> {
        serde_json::from_str(json).map_err(|e| MemoryError::BadSave(e.to_string()))
    }
}
