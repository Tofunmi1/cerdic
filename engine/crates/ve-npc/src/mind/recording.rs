//! Stable session recording and scripted response providers.
use super::{Request, Response};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Documented FNV-1a 64-bit hash for stable prompt keys.
pub(super) fn fnv(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325_u64;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub(super) struct Recorded {
    pub(super) npc: String,
    pub(super) prompt_hash: u64,
    pub(super) outcome: Option<String>,
}

/// JSON serializable record of completed provider outcomes.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Recording {
    pub(super) entries: Vec<Recorded>,
}

/// Invalid recording data.
#[derive(Clone, Debug, Error)]
pub enum MindError {
    /// JSON could not be decoded.
    #[error("bad recording: {0}")]
    BadRecording(String),
}

impl Recording {
    /// Serializes entries in stable field order.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".into())
    }

    /// Parses a recording.
    ///
    /// # Errors
    /// Returns [`MindError::BadRecording`] when the JSON is invalid.
    pub fn from_json(s: &str) -> Result<Self, MindError> {
        serde_json::from_str(s).map_err(|e| MindError::BadRecording(e.to_string()))
    }
}

/// Deterministic prompt-substring or recording-backed provider for tests and replay.
pub struct ScriptedProvider {
    rules: Vec<(String, String)>,
    replay: Vec<Recorded>,
    used: Vec<bool>,
}

impl ScriptedProvider {
    /// Creates an empty scripted provider.
    #[must_use]
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            replay: Vec::new(),
            used: Vec::new(),
        }
    }

    /// Adds a prompt substring rule.
    #[must_use]
    pub fn reply_when(mut self, needle: &str, reply: &str) -> Self {
        self.rules.push((needle.into(), reply.into()));
        self
    }

    /// Builds a provider from a session recording.
    #[must_use]
    pub fn from_recording(recording: &Recording) -> Self {
        Self {
            rules: Vec::new(),
            replay: recording.entries.clone(),
            used: vec![false; recording.entries.len()],
        }
    }

    /// Returns the first matching scripted or recorded response.
    pub fn respond(&mut self, request: &Request) -> Response {
        let outcome = if self.replay.is_empty() {
            self.rules
                .iter()
                .find(|(n, _)| request.prompt.contains(n))
                .map(|(_, r)| r.clone())
                .ok_or_else(|| "no scripted reply".to_owned())
        } else {
            self.replay
                .iter()
                .enumerate()
                .find(|(i, e)| {
                    !self.used[*i]
                        && e.npc == request.npc
                        && e.prompt_hash == fnv(request.prompt.as_bytes())
                })
                .map(|(i, e)| {
                    self.used[i] = true;
                    e.outcome.clone().unwrap_or_default()
                })
                .ok_or_else(|| "no scripted reply".to_owned())
        };
        Response {
            id: request.id,
            outcome,
        }
    }
}

impl Default for ScriptedProvider {
    fn default() -> Self {
        Self::new()
    }
}
