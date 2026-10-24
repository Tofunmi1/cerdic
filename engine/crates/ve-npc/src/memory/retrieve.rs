use super::utils::{cosine, decay};
use super::{Embedder, MemoryStore, RetrievalWeights, Retrieved};

impl MemoryStore {
    /// Ranks memories by weighted recency, importance, and cosine relevance.
    #[must_use]
    pub fn rank(
        &self,
        query: &str,
        now: f64,
        k: usize,
        weights: &RetrievalWeights,
        embedder: &dyn Embedder,
    ) -> Vec<Retrieved> {
        if k == 0 {
            return Vec::new();
        }
        let q = embedder.embed(query);
        let mut out: Vec<_> = self
            .memories
            .iter()
            .map(|m| {
                let recency = decay(now - m.last_access, weights.half_life);
                let relevance = cosine(&q, &m.embedding);
                Retrieved {
                    id: m.id,
                    score: weights.recency * recency
                        + weights.importance * m.importance
                        + weights.relevance * relevance,
                    recency,
                    importance: m.importance,
                    relevance,
                }
            })
            .collect();
        out.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.id.cmp(&b.id)));
        out.truncate(k);
        out
    }

    /// Ranks memories, then records access on exactly those returned.
    pub fn retrieve(
        &mut self,
        query: &str,
        now: f64,
        k: usize,
        weights: &RetrievalWeights,
        embedder: &dyn Embedder,
    ) -> Vec<Retrieved> {
        let found = self.rank(query, now, k, weights, embedder);
        for r in &found {
            if let Some(m) = self.memories.iter_mut().find(|m| m.id == r.id) {
                m.last_access = now;
                m.access_count = m.access_count.saturating_add(1);
            }
        }
        found
    }
}
