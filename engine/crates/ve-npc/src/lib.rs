//! NPC cognition for generated worlds (ADR 0063 and 0064).
#![deny(missing_docs)]

mod blackboard;
mod brain;
mod character;
mod dialogue;
mod fsm;
mod host;
mod htn;
mod intent;
mod memory;
mod mind;
mod prompt;
#[cfg(not(target_arch = "wasm32"))]
mod provider;
mod providers;
mod registry;
mod rng;
mod skills;
mod utility;

pub use blackboard::{Blackboard, Value};
pub use brain::{BehaviorTree, BrainError, Command, Ctx, Status};
pub use character::{
    Character, CheckResult, Expect, FallbackLine, Report, Script, ScriptLine, Summary, TurnReport,
    run_script,
};
pub use dialogue::{DialogueError, DialogueSession, SessionEvent, SessionState, Speaker, Turn};
pub use fsm::{Fsm, FsmTick};
pub use host::{FillerSpec, HostError, NpcHost, NpcSpec, SecretSpec};
pub use htn::{Domain, HtnError, Plan, PlanLimits, PlanRunner, RunStatus, Step};
pub use registry::Registry;
pub use rng::Rng;
pub use skills::{SkillStack, SkillTick};
pub use utility::{Choice, UtilityBrain};

/// One parser rejection diagnostic.
pub type Rejected = RejectedIntent;
/// Parsed and validated reply data.
pub type Parsed = ParsedReply;

pub use intent::{
    ArgType, ArgValue, Intent, IntentError, IntentSpec, ParsedReply, RejectedIntent, Vocabulary,
    parse_reply,
};
pub use memory::{
    Embedder, Fact, LexicalEmbedder, Memory, MemoryDraft, MemoryError, MemoryId, MemoryKind,
    MemoryStore, Relationship, RetrievalWeights, Retrieved, Source,
};
pub use mind::{
    Ask, Delivery, FallbackTable, Mind, MindBudget, MindError, Reason, Recording, Refusal, Request,
    RequestId, Response, ScriptedProvider,
};
pub use prompt::{Prompt, PromptBudget, PromptSections, assemble_prompt};
#[cfg(not(target_arch = "wasm32"))]
pub use provider::{CommandProvider, run_until_idle};
pub use providers::{Chain, MeterStats, Metered, Provider, ProviderError, ProviderRegistry};
