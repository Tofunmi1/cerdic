//! Memory types and deterministic in-memory store.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

/// Stable identifier for one memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MemoryId(pub u64);
/// Kind of episodic memory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryKind {
    /// A directly perceived observation.
    Observation,
    /// A world event.
    Event,
    /// A conversation.
    Dialogue,
    /// A higher-level conclusion.
    Reflection,
    /// An imagined experience.
    Imagined,
}
/// Provenance of a memory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    /// Directly witnessed.
    Witnessed,
    /// Reported by another character.
    Told {
        /// Immediate teller.
        from: String,
        /// Number of gossip hops.
        hops: u8,
    },
    /// Created by reflection.
    Reflected,
    /// Imagined.
    Imagined,
}
/// Input data for adding an episode.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryDraft {
    /// Memory category.
    pub kind: MemoryKind,
    /// Natural language content.
    pub text: String,
    /// Importance in [0, 1] after insertion.
    pub importance: f32,
    /// Optional place.
    pub place: Option<String>,
    /// Involved characters.
    pub participants: Vec<String>,
    /// Searchable tags.
    pub tags: Vec<String>,
    /// Provenance.
    pub source: Source,
}
impl MemoryDraft {
    /// Creates a witnessed draft with no place, participants, or tags.
    pub fn new(kind: MemoryKind, text: impl Into<String>, importance: f32) -> Self {
        Self {
            kind,
            text: text.into(),
            importance,
            place: None,
            participants: Vec::new(),
            tags: Vec::new(),
            source: Source::Witnessed,
        }
    }
}
/// Stored episodic memory.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Memory {
    /// Stable id.
    pub id: MemoryId,
    /// Category.
    pub kind: MemoryKind,
    /// Text.
    pub text: String,
    /// Clamped importance.
    pub importance: f32,
    /// Optional place.
    pub place: Option<String>,
    /// Participants.
    pub participants: Vec<String>,
    /// Tags.
    pub tags: Vec<String>,
    /// Provenance.
    pub source: Source,
    /// Creation time.
    pub time: f64,
    /// Most recent retrieval time.
    pub last_access: f64,
    /// Number of retrievals.
    pub access_count: u32,
    /// Protected from capacity forgetting.
    pub pinned: bool,
    /// Lexical embedding.
    pub embedding: Vec<f32>,
}
/// Stored fact with confidence and timestamp.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fact {
    /// Fact value.
    pub value: String,
    /// Confidence clamped to [0, 1].
    pub confidence: f32,
    /// Set time.
    pub time: f64,
}
/// Relationship values for another character.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Relationship {
    /// Affinity in [-1, 1].
    pub affinity: f32,
    /// Trust in [-1, 1].
    pub trust: f32,
    /// Familiarity in [0, 1].
    pub familiarity: f32,
}
/// Retrieval score and its components.
#[derive(Clone, Debug)]
pub struct Retrieved {
    /// Memory id.
    pub id: MemoryId,
    /// Weighted score.
    pub score: f32,
    /// Recency component.
    pub recency: f32,
    /// Importance component.
    pub importance: f32,
    /// Embedding similarity.
    pub relevance: f32,
}
/// Weights for memory ranking.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct RetrievalWeights {
    /// Recency weight.
    pub recency: f32,
    /// Importance weight.
    pub importance: f32,
    /// Relevance weight.
    pub relevance: f32,
    /// Recency half-life in seconds.
    pub half_life: f64,
}
impl Default for RetrievalWeights {
    fn default() -> Self {
        Self {
            recency: 1.0,
            importance: 1.0,
            relevance: 1.0,
            half_life: 3600.0,
        }
    }
}
/// Memory operation failures.
#[derive(Debug, Error)]
pub enum MemoryError {
    /// Gossip exceeded its hop limit.
    #[error("gossip has {hops} hops, maximum is {max}")]
    TooManyHops {
        /// Proposed hop count.
        hops: u8,
        /// Hop limit.
        max: u8,
    },
    /// Requested memory does not exist.
    #[error("unknown memory {0:?}")]
    UnknownMemory(MemoryId),
    /// Saved representation is invalid.
    #[error("bad memory save: {0}")]
    BadSave(String),
}
/// Per-character memory, facts, and relationships.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryStore {
    pub(super) capacity: usize,
    pub(super) next_id: u64,
    pub(super) memories: Vec<Memory>,
    pub(super) facts: BTreeMap<String, Fact>,
    pub(super) relationships: BTreeMap<String, Relationship>,
    pub(super) reflected_at: f64,
}
impl MemoryStore {
    /// Creates an empty store with a memory capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            next_id: 1,
            memories: Vec::new(),
            facts: BTreeMap::new(),
            relationships: BTreeMap::new(),
            reflected_at: f64::MIN,
        }
    }
    /// Number of memories currently stored.
    #[must_use]
    pub fn len(&self) -> usize {
        self.memories.len()
    }
    /// Whether the store contains no memories.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.memories.is_empty()
    }
    /// Adds a memory and forgets the least valuable unpinned entry if over capacity.
    pub fn add(
        &mut self,
        draft: MemoryDraft,
        now: f64,
        embedder: &dyn super::Embedder,
    ) -> MemoryId {
        let id = MemoryId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        let importance = if draft.importance.is_nan() {
            0.0
        } else {
            draft.importance.clamp(0.0, 1.0)
        };
        let embedding = embedder.embed(&draft.text);
        self.memories.push(Memory {
            id,
            kind: draft.kind,
            text: draft.text,
            importance,
            place: draft.place,
            participants: draft.participants,
            tags: draft.tags,
            source: draft.source,
            time: now,
            last_access: now,
            access_count: 0,
            pinned: false,
            embedding,
        });
        self.forget_if_needed(now);
        id
    }
    pub(super) fn forget_if_needed(&mut self, now: f64) {
        if self.memories.len() <= self.capacity {
            return;
        }
        let victim = self
            .memories
            .iter()
            .filter(|m| !m.pinned)
            .min_by(|a, b| keep(a, now).total_cmp(&keep(b, now)).then(a.id.cmp(&b.id)))
            .map(|m| m.id);
        if let Some(id) = victim {
            self.memories.retain(|m| m.id != id);
        }
    }
    /// Gets a memory by id.
    #[must_use]
    pub fn get(&self, id: MemoryId) -> Option<&Memory> {
        self.memories.iter().find(|m| m.id == id)
    }
    /// Pins or unpins a memory; returns false if the id is unknown.
    pub fn pin(&mut self, id: MemoryId) -> bool {
        if let Some(m) = self.memories.iter_mut().find(|m| m.id == id) {
            m.pinned = true;
            true
        } else {
            false
        }
    }
    /// Returns newest memories first, with higher id breaking time ties.
    #[must_use]
    pub fn recent(&self, n: usize) -> Vec<&Memory> {
        let mut v: Vec<_> = self.memories.iter().collect();
        v.sort_by(|a, b| b.time.total_cmp(&a.time).then(b.id.cmp(&a.id)));
        v.truncate(n);
        v
    }
}
fn keep(m: &Memory, now: f64) -> f64 {
    f64::from(m.importance) * 0.5_f64.powf((now - m.last_access) / 86400.0)
}
