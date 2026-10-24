//! Stable host persistence.
use super::{HostError, NpcHost, NpcSpec};
use crate::MemoryStore;
use crate::{LexicalEmbedder, Mind, MindBudget, RetrievalWeights, Vocabulary};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize)]
struct Save {
    version: u8,
    retrieval_count: usize,
    npcs: BTreeMap<String, SavedNpc>,
    #[serde(default = "default_threshold")]
    reflection_threshold: f32,
    #[serde(default)]
    auto_reflect: bool,
    #[serde(default)]
    retries: BTreeMap<String, u32>,
    #[serde(default)]
    fillers: BTreeMap<String, super::FillerSpec>,
    #[serde(default)]
    filler_counts: BTreeMap<String, usize>,
}
fn default_threshold() -> f32 {
    1.5
}
#[derive(Serialize, Deserialize)]
struct SavedNpc {
    spec: NpcSpec,
    memory: MemoryStore,
    session_json: String,
    events: Vec<crate::SessionEvent>,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    secrets: Vec<super::SecretSpec>,
    #[serde(default)]
    examples: Vec<(String, String)>,
    #[serde(default)]
    deflections: Vec<String>,
    #[serde(default)]
    leaks_blocked: u32,
}

pub(super) fn to_json(host: &NpcHost) -> String {
    serde_json::to_string(&Save {
        version: 1,
        retrieval_count: host.retrieval_count,
        npcs: host
            .npcs
            .iter()
            .map(|(name, npc)| {
                (
                    name.clone(),
                    SavedNpc {
                        spec: npc.spec.clone(),
                        memory: npc.memory.clone(),
                        session_json: npc.session.to_json(),
                        events: npc.events.clone(),
                        provider: npc.provider.clone(),
                        secrets: npc.secrets.clone(),
                        examples: npc.examples.clone(),
                        deflections: npc.deflections.clone(),
                        leaks_blocked: npc.leaks_blocked,
                    },
                )
            })
            .collect(),
        reflection_threshold: host.reflection_threshold,
        auto_reflect: host.auto_reflect,
        retries: host.retries.clone(),
        fillers: host.fillers.clone(),
        filler_counts: host.filler_counts.clone(),
    })
    .unwrap_or_else(|_| "{}".into())
}

pub(super) fn from_json(json: &str, budget: MindBudget) -> Result<NpcHost, HostError> {
    let save: Save = serde_json::from_str(json).map_err(|e| HostError::BadSave(e.to_string()))?;
    if save.version != 1 {
        return Err(HostError::BadSave("unsupported version".into()));
    }
    let mut npcs = BTreeMap::new();
    for (name, npc) in save.npcs {
        if name != npc.spec.name {
            return Err(HostError::BadSave(
                "NPC key does not match spec name".into(),
            ));
        }
        let session = crate::DialogueSession::from_json(&npc.session_json)
            .map_err(|e| HostError::BadSave(e.to_string()))?;
        npcs.insert(
            name,
            super::NpcData {
                spec: npc.spec,
                memory: npc.memory,
                session,
                events: npc.events,
                provider: npc.provider,
                secrets: npc.secrets,
                examples: npc.examples,
                deflections: npc.deflections,
                leaks_blocked: npc.leaks_blocked,
            },
        );
    }
    let vocabulary = Vocabulary::with_builtins();
    Ok(NpcHost {
        mind: Mind::new(vocabulary.clone(), budget),
        vocabulary,
        npcs,
        embedder: LexicalEmbedder::new(256),
        weights: RetrievalWeights::default(),
        retrieval_count: save.retrieval_count,
        requests: BTreeMap::new(),
        last_deliveries: BTreeMap::new(),
        attempts: BTreeMap::new(),
        request_meta: BTreeMap::new(),
        retries: save.retries,
        fillers: save.fillers,
        filler_counts: save.filler_counts,
        reflection_outstanding: std::collections::BTreeSet::new(),
        reflection_threshold: save.reflection_threshold,
        auto_reflect: save.auto_reflect,
    })
}
