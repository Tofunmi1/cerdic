//! NPC authoring data and host operation errors.
use crate::MemoryError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A private NPC fact released at a minimum player trust level.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SecretSpec {
    /// The content shown after the secret unlocks.
    pub text: String,
    /// Required relationship trust, compared directly to the stored trust.
    pub min_trust: f32,
    /// Lowercase-insensitive substring markers used by the leak guard.
    pub markers: Vec<String>,
}

/// Authored identity and knowledge for one NPC.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NpcSpec {
    /// Stable NPC name.
    pub name: String,
    /// Persona prompt.
    pub persona: String,
    /// Behavioral rules.
    pub rules: String,
    /// Current goals.
    pub goals: Vec<String>,
    /// Granted world knowledge.
    pub world_facts: Vec<String>,
}

/// Failure returned by host operations.
#[derive(Debug, Error)]
pub enum HostError {
    /// NPC name is not registered.
    #[error("unknown NPC: {0}")]
    UnknownNpc(String),
    /// NPC name is already registered.
    #[error("duplicate NPC: {0}")]
    Duplicate(String),
    /// Saved host data is invalid.
    #[error("bad host save: {0}")]
    BadSave(String),
    /// JSON command is invalid.
    #[error("bad command: {0}")]
    BadCommand(String),
    /// Memory operation failed.
    #[error(transparent)]
    Memory(#[from] MemoryError),
}
