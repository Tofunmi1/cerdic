//! Deterministic per-NPC memory stream, retrieval, facts, and relationships.
#[path = "memory/embed.rs"]
mod embed;
#[path = "memory/facts.rs"]
mod facts;
#[path = "memory/gossip.rs"]
mod gossip;
#[path = "memory/relationships.rs"]
mod relationships;
#[path = "memory/retrieve.rs"]
mod retrieve;
#[path = "memory/save.rs"]
mod save;
#[path = "memory/store.rs"]
mod store;
#[path = "memory/utils.rs"]
mod utils;

pub use embed::{Embedder, LexicalEmbedder};
pub use store::{
    Fact, Memory, MemoryDraft, MemoryError, MemoryId, MemoryKind, MemoryStore, Relationship,
    RetrievalWeights, Retrieved, Source,
};
