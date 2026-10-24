//! Deterministic asynchronous scheduler for NPC mind requests.
use crate::{Intent, Rejected, Vocabulary, parse_reply};
use std::collections::BTreeMap;
mod fallback;
mod queue;
mod recording;
pub use fallback::FallbackTable;
use fallback::fallback_intents;
pub use recording::{MindError, Recording, ScriptedProvider};
use recording::{Recorded, fnv};

/// Stable request identifier, allocated from one within each mind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequestId(pub u64);
/// Why a mind request was issued.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// A player spoke to the NPC.
    PlayerSpoke,
    /// A salient world event occurred.
    Event,
    /// The fast brain requested deliberation.
    BrainRequest,
    /// A reflection is due.
    Reflection,
    /// Ambient idle thought.
    Idle,
}
impl Reason {
    fn priority(self) -> u8 {
        match self {
            Self::PlayerSpoke => 100,
            Self::Event => 60,
            Self::BrainRequest => 50,
            Self::Reflection => 40,
            Self::Idle => 10,
        }
    }
}
/// Limits for request admission and lifetime.
#[derive(Clone, Copy, Debug)]
pub struct MindBudget {
    /// Maximum asks per NPC in a rolling minute.
    pub calls_per_minute: u32,
    /// Maximum admitted requests at once.
    pub max_concurrent: usize,
    /// Maximum estimated prompt tokens in a rolling minute.
    pub tokens_per_minute: u32,
    /// Request lifetime in seconds.
    pub timeout_secs: f64,
}
impl Default for MindBudget {
    fn default() -> Self {
        Self {
            calls_per_minute: 6,
            max_concurrent: 4,
            tokens_per_minute: 20_000,
            timeout_secs: 8.0,
        }
    }
}
/// Request sent to an external provider.
#[derive(Clone, Debug)]
pub struct Request {
    /// Request id.
    pub id: RequestId,
    /// NPC name.
    pub npc: String,
    /// Prompt text.
    pub prompt: String,
    /// Request reason.
    pub reason: Reason,
    /// Scheduling priority.
    pub priority: u8,
    /// Issue time in seconds.
    pub issued_at: f64,
}
/// Provider response, delivered asynchronously.
#[derive(Clone, Debug)]
pub struct Response {
    /// Request id.
    pub id: RequestId,
    /// Raw outcome text or provider error.
    pub outcome: Result<String, String>,
}
/// Result of asking a mind to deliberate.
#[derive(Clone, Debug, PartialEq)]
pub enum Ask {
    /// Admitted for provider dispatch.
    Sent(RequestId),
    /// Accepted but waiting for a concurrent slot.
    Queued(RequestId),
    /// Not admitted due to a budget.
    Refused(Refusal),
}
/// Admission refusal reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// NPC call limit reached.
    RateLimited,
    /// Scene token limit reached.
    OverTokenBudget,
}
/// Validated result for an NPC request.
#[derive(Clone, Debug)]
pub struct Delivery {
    /// Completed request id.
    pub request: RequestId,
    /// NPC name.
    pub npc: String,
    /// Validated intents.
    pub intents: Vec<Intent>,
    /// Parser diagnostics.
    pub rejected: Vec<Rejected>,
    /// True when fallback handled the request.
    pub fallback: bool,
}
/// Scheduler state for requests, fallbacks, and optional replay recording.
pub struct Mind {
    vocab: Vocabulary,
    budget: MindBudget,
    next: u64,
    entries: BTreeMap<RequestId, queue::Entry>,
    fallbacks: BTreeMap<String, FallbackTable>,
    recording: Option<Recording>,
}
impl Mind {
    /// Creates a scheduler with the allowed intent vocabulary and budgets.
    #[must_use]
    pub fn new(vocab: Vocabulary, budget: MindBudget) -> Self {
        Self {
            vocab,
            budget,
            next: 1,
            entries: BTreeMap::new(),
            fallbacks: BTreeMap::new(),
            recording: None,
        }
    }
    /// Sets a named NPC's fallback lines.
    pub fn set_fallback(&mut self, npc: &str, table: FallbackTable) {
        self.fallbacks.insert(npc.into(), table);
    }
    /// Registers an additional allowed intent.
    ///
    /// # Errors
    /// Returns an error when the intent name is invalid or already registered.
    pub fn add_intent(&mut self, spec: crate::IntentSpec) -> Result<(), crate::IntentError> {
        self.vocab.register(spec)
    }
    /// Builds the configured fallback delivery for an immediately refused ask.
    #[must_use]
    pub fn immediate_fallback(&self, npc: &str, reason: Reason) -> Delivery {
        let request = Request {
            id: RequestId(0),
            npc: npc.into(),
            prompt: String::new(),
            reason,
            priority: reason.priority(),
            issued_at: 0.0,
        };
        let intents = fallback_intents(&self.fallbacks, &request, &self.vocab);
        Delivery {
            request: RequestId(0),
            npc: npc.into(),
            intents,
            rejected: Vec::new(),
            fallback: true,
        }
    }
    /// Completes a known request, validating its response or using a fallback.
    pub fn deliver(&mut self, response: Response, _now: f64) -> Option<Delivery> {
        let entry = self.entries.remove(&response.id)?;
        let prompt_hash = fnv(entry.request.prompt.as_bytes());
        let (text, parse) = match response.outcome {
            Ok(t) => {
                let p = parse_reply(&t, &self.vocab);
                (Some(t), p)
            }
            Err(_) => (None, crate::ParsedReply::default()),
        };
        let fallback = text.is_none() || parse.intents.is_empty();
        let delivery = self.make_delivery(entry.request, fallback, parse.intents, parse.rejected);
        if let Some(recording) = &mut self.recording {
            recording.entries.push(Recorded {
                npc: delivery.npc.clone(),
                prompt_hash,
                outcome: text,
            });
        }
        self.admit_waiting();
        Some(delivery)
    }
    fn make_delivery(
        &self,
        request: Request,
        fallback: bool,
        parsed_intents: Vec<Intent>,
        rejected: Vec<Rejected>,
    ) -> Delivery {
        let intents = if fallback {
            fallback_intents(&self.fallbacks, &request, &self.vocab)
        } else {
            parsed_intents
        };
        Delivery {
            request: request.id,
            npc: request.npc,
            intents,
            rejected,
            fallback,
        }
    }
    /// Expires overdue requests in id order with fallback deliveries.
    pub fn tick(&mut self, now: f64) -> Vec<Delivery> {
        let ids: Vec<_> = self
            .entries
            .values()
            .filter(|e| now - e.request.issued_at > self.budget.timeout_secs)
            .map(|e| e.request.id)
            .collect();
        let out = ids
            .into_iter()
            .filter_map(|id| {
                let e = self.entries.remove(&id)?;
                Some(self.make_delivery(e.request, true, Vec::new(), Vec::new()))
            })
            .collect();
        self.admit_waiting();
        out
    }
    /// Counts pending admitted and queued requests.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.entries.len()
    }
    /// Silently cancels every pending request for an NPC.
    pub fn cancel_npc(&mut self, npc: &str) {
        self.entries.retain(|_, e| e.request.npc != npc);
        self.admit_waiting();
    }
    /// Starts a fresh session recording.
    pub fn start_recording(&mut self) {
        self.recording = Some(Recording::default());
    }
    /// Clones the current recording, or an empty recording when disabled.
    #[must_use]
    pub fn recording(&self) -> Recording {
        self.recording.clone().unwrap_or_default()
    }
}
