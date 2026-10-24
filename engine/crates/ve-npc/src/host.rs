//! High-level NPC host joining memory, mind, prompts, dialogue, and commands.
use crate::{
    DialogueSession, FallbackTable, IntentError, IntentSpec, LexicalEmbedder, MemoryDraft,
    MemoryId, MemoryKind, MemoryStore, Mind, MindBudget, PromptBudget, PromptSections, Reason,
    Refusal, Request, RequestId, Response, RetrievalWeights, SessionEvent, Vocabulary,
    assemble_prompt,
};
use std::collections::BTreeMap;
use std::fmt::Write as _;

mod age;
mod apply;
mod commands;
mod reflect;
mod save;
mod spec;
pub use spec::{HostError, NpcSpec, SecretSpec};

/// Filler lines to play while a player-spoke request is pending.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FillerSpec {
    /// Delay before the first filler is emitted.
    pub after_secs: f64,
    /// Lines, selected in deterministic rotation.
    pub lines: Vec<String>,
}

pub(super) struct NpcData {
    pub spec: NpcSpec,
    pub memory: MemoryStore,
    pub session: DialogueSession,
    pub events: Vec<SessionEvent>,
    pub provider: Option<String>,
    pub secrets: Vec<SecretSpec>,
    pub examples: Vec<(String, String)>,
    pub deflections: Vec<String>,
    pub leaks_blocked: u32,
}

/// Coordinates deterministic NPC cognition for a scene.
pub struct NpcHost {
    pub(super) mind: Mind,
    pub(super) vocabulary: Vocabulary,
    pub(super) npcs: BTreeMap<String, NpcData>,
    pub(super) embedder: LexicalEmbedder,
    pub(super) weights: RetrievalWeights,
    pub(super) retrieval_count: usize,
    pub(super) requests: BTreeMap<RequestId, String>,
    pub(super) attempts: BTreeMap<RequestId, u32>,
    pub(super) request_meta: BTreeMap<RequestId, RequestMeta>,
    pub(super) retries: BTreeMap<String, u32>,
    pub(super) fillers: BTreeMap<String, FillerSpec>,
    pub(super) filler_counts: BTreeMap<String, usize>,
    pub(super) reflection_outstanding: std::collections::BTreeSet<String>,
    pub(super) reflection_threshold: f32,
    pub(super) auto_reflect: bool,
    pub(super) last_deliveries: BTreeMap<String, crate::Delivery>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RequestKind {
    Player,
    Reflection,
}
#[derive(Clone)]
pub(super) struct RequestMeta {
    pub issued_at: f64,
    pub kind: RequestKind,
    pub filler_sent: bool,
    pub prompt: String,
}
impl NpcHost {
    /// Creates an empty host with built-in intents and the requested scheduler budget.
    #[must_use]
    pub fn new(budget: MindBudget) -> Self {
        let vocabulary = Vocabulary::with_builtins();
        Self {
            mind: Mind::new(vocabulary.clone(), budget),
            vocabulary,
            npcs: BTreeMap::new(),
            embedder: LexicalEmbedder::new(256),
            weights: RetrievalWeights::default(),
            retrieval_count: 5,
            requests: BTreeMap::new(),
            attempts: BTreeMap::new(),
            request_meta: BTreeMap::new(),
            retries: BTreeMap::new(),
            fillers: BTreeMap::new(),
            filler_counts: BTreeMap::new(),
            reflection_outstanding: std::collections::BTreeSet::new(),
            reflection_threshold: 1.5,
            auto_reflect: false,
            last_deliveries: BTreeMap::new(),
        }
    }
    /// Registers an NPC with an empty 512-memory store and dialogue session.
    ///
    /// # Errors
    /// Returns [`HostError::Duplicate`] if the name is already registered.
    pub fn register(&mut self, spec: NpcSpec) -> Result<(), HostError> {
        if self.npcs.contains_key(&spec.name) {
            return Err(HostError::Duplicate(spec.name));
        }
        let name = spec.name.clone();
        self.npcs.insert(
            name.clone(),
            NpcData {
                spec,
                memory: MemoryStore::new(512),
                session: DialogueSession::new(&name, 0.0),
                events: Vec::new(),
                provider: None,
                secrets: Vec::new(),
                examples: Vec::new(),
                deflections: Vec::new(),
                leaks_blocked: 0,
            },
        );
        Ok(())
    }
    /// Sets fallback lines for an NPC.
    pub fn set_fallback(&mut self, npc: &str, table: FallbackTable) {
        self.mind.set_fallback(npc, table);
    }
    /// Adds a trust-gated secret for a registered NPC.
    ///
    /// # Errors
    /// Returns [`HostError::UnknownNpc`] if the NPC is not registered.
    pub fn add_secret(&mut self, npc: &str, secret: SecretSpec) -> Result<(), HostError> {
        self.npcs
            .get_mut(npc)
            .ok_or_else(|| HostError::UnknownNpc(npc.into()))?
            .secrets
            .push(secret);
        Ok(())
    }
    /// Sets up to eight worked dialogue examples for a registered NPC.
    ///
    /// # Errors
    /// Returns [`HostError::UnknownNpc`] if the NPC is not registered.
    pub fn set_examples(
        &mut self,
        npc: &str,
        examples: Vec<(String, String)>,
    ) -> Result<(), HostError> {
        self.npcs
            .get_mut(npc)
            .ok_or_else(|| HostError::UnknownNpc(npc.into()))?
            .examples = examples.into_iter().take(8).collect();
        Ok(())
    }
    /// Sets deterministic leak deflections; an empty list uses the default line.
    /// Unknown NPC names are ignored.
    pub fn set_deflections(&mut self, npc: &str, lines: Vec<String>) {
        if let Some(data) = self.npcs.get_mut(npc) {
            data.deflections = lines;
        }
    }
    /// Returns the number of blocked leaks for an NPC, or zero when unknown.
    #[must_use]
    pub fn leaks_blocked(&self, npc: &str) -> u32 {
        self.npcs.get(npc).map_or(0, |data| data.leaks_blocked)
    }
    /// Assigns a named provider to an NPC; registration is checked when a request is pumped.
    ///
    /// # Errors
    /// Returns [`HostError::UnknownNpc`] when the NPC is not registered.
    pub fn set_provider(&mut self, npc: &str, provider_name: &str) -> Result<(), HostError> {
        let data = self
            .npcs
            .get_mut(npc)
            .ok_or_else(|| HostError::UnknownNpc(npc.into()))?;
        data.provider = Some(provider_name.into());
        Ok(())
    }

    /// Dispatches up to `max_requests` pending requests through named or default providers.
    pub fn pump(
        &mut self,
        providers: &mut crate::ProviderRegistry,
        now: f64,
        max_requests: usize,
    ) -> usize {
        let requests = self.mind.take_outbox_limit(max_requests);
        let mut answered = 0;
        for request in requests {
            self.requests.insert(request.id, request.npc.clone());
            self.attempts.entry(request.id).or_insert(0);
            self.request_meta.entry(request.id).or_insert(RequestMeta {
                issued_at: request.issued_at,
                kind: if request.reason == Reason::Reflection {
                    RequestKind::Reflection
                } else {
                    RequestKind::Player
                },
                filler_sent: false,
                prompt: request.prompt.clone(),
            });
            let selected = self
                .npcs
                .get(&request.npc)
                .and_then(|npc| npc.provider.clone());
            let response = match selected.as_deref() {
                Some(name) => providers.get_mut(name).map_or_else(
                    || crate::Response {
                        id: request.id,
                        outcome: Err(format!("no provider: {name}")),
                    },
                    |p| p.respond(&request),
                ),
                None => providers.default_provider().map_or_else(
                    || crate::Response {
                        id: request.id,
                        outcome: Err("no provider: default".into()),
                    },
                    |p| p.respond(&request),
                ),
            };
            let _ = self.deliver(response, now);
            answered += 1;
        }
        answered
    }
    /// Adds an allowed custom intent.
    ///
    /// # Errors
    /// Returns an [`IntentError`] when the specification is invalid or duplicated.
    pub fn add_intent(&mut self, spec: IntentSpec) -> Result<(), IntentError> {
        self.vocabulary.register(spec.clone())?;
        self.mind.add_intent(spec)
    }
    /// Returns an NPC memory store when registered.
    #[must_use]
    pub fn memory(&self, npc: &str) -> Option<&MemoryStore> {
        self.npcs.get(npc).map(|n| &n.memory)
    }
    /// Returns an NPC dialogue session when registered.
    #[must_use]
    pub fn session(&self, npc: &str) -> Option<&DialogueSession> {
        self.npcs.get(npc).map(|n| &n.session)
    }
    /// Returns the latest completed mind delivery for a registered NPC.
    #[must_use]
    pub fn last_delivery(&self, npc: &str) -> Option<&crate::Delivery> {
        self.last_deliveries.get(npc)
    }
    /// Adds a witnessed observation.
    ///
    /// # Errors
    /// Returns [`HostError::UnknownNpc`] when the NPC is not registered.
    pub fn observe(
        &mut self,
        npc: &str,
        text: &str,
        importance: f32,
        now: f64,
    ) -> Result<MemoryId, HostError> {
        let data = self
            .npcs
            .get_mut(npc)
            .ok_or_else(|| HostError::UnknownNpc(npc.into()))?;
        Ok(data.memory.add(
            MemoryDraft::new(MemoryKind::Observation, text, importance),
            now,
            &self.embedder,
        ))
    }
    /// Records a player line, retrieves relevant memories, and schedules the NPC mind.
    ///
    /// # Errors
    /// Returns an error when the NPC is unknown or its dialogue session is closed.
    pub fn player_says(
        &mut self,
        npc: &str,
        text: &str,
        now: f64,
    ) -> Result<Option<Refusal>, HostError> {
        let data = self
            .npcs
            .get_mut(npc)
            .ok_or_else(|| HostError::UnknownNpc(npc.into()))?;
        let retrieved = data.memory.retrieve(
            text,
            now,
            self.retrieval_count,
            &self.weights,
            &self.embedder,
        );
        let memories = retrieved
            .iter()
            .filter_map(|r| data.memory.get(r.id))
            .map(|m| format!("({}) {}", age::format_age(now - m.time), m.text))
            .collect();
        record_player_line(data, text, now, &self.embedder)?;
        let trust = data.memory.relationship("player").trust;
        let secrets = data
            .secrets
            .iter()
            .filter(|s| trust >= s.min_trust)
            .map(|s| format!("Secrets you hold: {}", s.text))
            .collect();
        let locked = data.secrets.iter().any(|s| trust < s.min_trust);
        let mut rules = data.spec.rules.clone();
        if locked {
            rules.push_str("\nYou have private matters you will not discuss; if asked, steer the conversation elsewhere politely.");
        }
        if !data.examples.is_empty() {
            rules.push_str("\n\n## Examples\n");
            for (q, a) in &data.examples {
                let _ = writeln!(rules, "player: {q}\n{}: {a}", data.spec.name);
            }
        }
        let sections = PromptSections {
            persona: data.spec.persona.clone(),
            rules,
            world: data.spec.world_facts.clone(),
            goals: data.spec.goals.clone(),
            state: secrets,
            memories,
            conversation: data.session.conversation(12),
            situation: format!("The player says: \"{text}\""),
        };
        let prompt = assemble_prompt(
            &sections,
            &self.vocabulary,
            &PromptBudget { max_chars: 12_000 },
        );
        let answer = self.mind.ask(npc, Reason::PlayerSpoke, &prompt.text, now);
        data.events.extend(data.session.drain_events());
        match answer {
            crate::Ask::Sent(id) | crate::Ask::Queued(id) => {
                self.requests.insert(id, npc.into());
                self.attempts.insert(id, 0);
                self.request_meta.insert(
                    id,
                    RequestMeta {
                        issued_at: now,
                        kind: RequestKind::Player,
                        filler_sent: false,
                        prompt: prompt.text.clone(),
                    },
                );
                Ok(None)
            }
            crate::Ask::Refused(r) => {
                let delivery = self.mind.immediate_fallback(npc, Reason::PlayerSpoke);
                data.session
                    .apply_delivery(&delivery, now)
                    .map_err(|e| HostError::BadCommand(e.to_string()))?;
                data.events.extend(data.session.drain_events());
                Ok(Some(r))
            }
        }
    }
    /// Sets the number of retrieved memories in each prompt.
    pub fn set_retrieval_count(&mut self, count: usize) {
        self.retrieval_count = count;
    }
    /// Returns pending model requests and remembers request ownership.
    pub fn take_outbox(&mut self) -> Vec<Request> {
        let requests = self.mind.take_outbox();
        for request in &requests {
            self.requests.insert(request.id, request.npc.clone());
            self.attempts.entry(request.id).or_insert(0);
            self.request_meta.entry(request.id).or_insert(RequestMeta {
                issued_at: request.issued_at,
                kind: if request.reason == Reason::Reflection {
                    RequestKind::Reflection
                } else {
                    RequestKind::Player
                },
                filler_sent: false,
                prompt: request.prompt.clone(),
            });
        }
        requests
    }
    /// Completes a pending response, applying dialogue and memory effects.
    pub fn deliver(&mut self, response: Response, now: f64) -> Option<crate::Delivery> {
        let npc = self.requests.remove(&response.id)?;
        let meta = self.request_meta.remove(&response.id);
        let attempt = self.attempts.remove(&response.id).unwrap_or(0);
        let successful_provider = response.outcome.is_ok();
        let retry_prompt = match (&response.outcome, meta.as_ref()) {
            (Ok(text), Some(info))
                if attempt == 0
                    && info.kind == RequestKind::Player
                    && crate::parse_reply(text, &self.vocabulary)
                        .intents
                        .is_empty() =>
            {
                Some(format!(
                    "{}\n\n## Correction\nYour last reply was not valid. Reply again with ONLY ONE JSON object shaped like the example in the reply format.",
                    info.prompt
                ))
            }
            _ => None,
        };
        let retry_ask = retry_prompt
            .as_ref()
            .map(|prompt| self.mind.ask(&npc, Reason::PlayerSpoke, prompt, now));
        let mut delivery = self.mind.deliver(response, now)?;
        let reflection_delivery = meta
            .as_ref()
            .is_some_and(|m| m.kind == RequestKind::Reflection);
        if reflection_delivery {
            self.reflection_outstanding.remove(&npc);
            let applied_reflection = delivery.intents.iter().any(|i| i.name == "reflect");
            if applied_reflection && let Some(data) = self.npcs.get_mut(&npc) {
                data.memory.mark_reflected(now);
            }
            if !applied_reflection {
                delivery.intents.clear();
                delivery.fallback = false;
            }
        }
        if successful_provider
            && delivery.fallback
            && let Some(crate::Ask::Sent(id) | crate::Ask::Queued(id)) = retry_ask
        {
            let prompt = retry_prompt.unwrap_or_default();
            self.requests.insert(id, npc.clone());
            self.attempts.insert(id, 1);
            self.request_meta.insert(
                id,
                RequestMeta {
                    issued_at: now,
                    kind: RequestKind::Player,
                    filler_sent: false,
                    prompt,
                },
            );
            *self.retries.entry(npc).or_default() += 1;
            return None;
        }
        apply::guard(self, &npc, &mut delivery);
        apply::apply(self, &npc, &delivery, now, !reflection_delivery);
        self.last_deliveries.insert(npc, delivery.clone());
        Some(delivery)
    }
    /// Expires overdue requests and applies fallback deliveries.
    pub fn tick(&mut self, now: f64) -> Vec<crate::Delivery> {
        self.emit_fillers(now);
        let mut deliveries = self.mind.tick(now);
        deliveries.retain(|d| {
            let reflection = self
                .request_meta
                .get(&d.request)
                .is_some_and(|m| m.kind == RequestKind::Reflection);
            if reflection {
                self.reflection_outstanding.remove(&d.npc);
                self.requests.remove(&d.request);
                self.request_meta.remove(&d.request);
                self.attempts.remove(&d.request);
            }
            !reflection
        });
        for d in &deliveries {
            self.requests.remove(&d.request);
            self.request_meta.remove(&d.request);
            self.attempts.remove(&d.request);
            apply::apply(self, &d.npc, d, now, true);
            self.last_deliveries.insert(d.npc.clone(), d.clone());
        }
        if self.auto_reflect {
            let names: Vec<_> = self.npcs.keys().cloned().collect();
            for name in names {
                let _ = self.maybe_reflect(&name, now);
            }
        }
        deliveries
    }
    /// Sets the per-NPC filler configuration. Unknown NPC names are ignored.
    pub fn set_filler(&mut self, npc: &str, spec: FillerSpec) {
        if self.npcs.contains_key(npc) {
            self.fillers.insert(npc.into(), spec);
        }
    }
    /// Returns how many repair retries have been admitted for an NPC.
    #[must_use]
    pub fn retries_used(&self, npc: &str) -> u32 {
        self.retries.get(npc).copied().unwrap_or(0)
    }
    /// Sets the accumulated importance required to request a reflection.
    pub fn set_reflection_threshold(&mut self, value: f32) {
        self.reflection_threshold = value;
    }
    /// Enables or disables automatic reflection checks during ticks.
    pub fn set_auto_reflect(&mut self, value: bool) {
        self.auto_reflect = value;
    }
    /// Requests a reflection when one is due and none is already pending.
    ///
    /// # Errors
    /// Returns [`HostError::UnknownNpc`] when `npc` is not registered.
    pub fn maybe_reflect(&mut self, npc: &str, now: f64) -> Result<bool, HostError> {
        reflect::maybe_reflect(self, npc, now)
    }
    /// Shares a memory between registered NPCs with gossip provenance.
    ///
    /// # Errors
    /// Returns an unknown-NPC or memory-sharing error.
    pub fn gossip(
        &mut self,
        from: &str,
        to: &str,
        id: MemoryId,
        now: f64,
        max_hops: u8,
    ) -> Result<MemoryId, HostError> {
        if !self.npcs.contains_key(from) {
            return Err(HostError::UnknownNpc(from.into()));
        }
        if !self.npcs.contains_key(to) {
            return Err(HostError::UnknownNpc(to.into()));
        }
        let from_store = self
            .npcs
            .get(from)
            .map(|n| n.memory.clone())
            .ok_or_else(|| HostError::UnknownNpc(from.into()))?;
        let target = &mut self
            .npcs
            .get_mut(to)
            .ok_or_else(|| HostError::UnknownNpc(to.into()))?
            .memory;
        Ok(from_store.share(id, target, from, now, &self.embedder, max_hops)?)
    }
    /// Drains ordered session events for an NPC.
    ///
    /// # Errors
    /// Returns [`HostError::UnknownNpc`] when the NPC is not registered.
    pub fn drain_events(&mut self, npc: &str) -> Result<Vec<SessionEvent>, HostError> {
        let data = self
            .npcs
            .get_mut(npc)
            .ok_or_else(|| HostError::UnknownNpc(npc.into()))?;
        let mut events = data.session.drain_events();
        events.append(&mut data.events);
        Ok(events)
    }
    /// Serializes stable NPC state in name order, excluding scheduler queues.
    #[must_use]
    pub fn to_json(&self) -> String {
        save::to_json(self)
    }
    /// Restores host state with a fresh scheduler.
    ///
    /// # Errors
    /// Returns [`HostError::BadSave`] when the JSON is invalid or inconsistent.
    pub fn from_json(json: &str, budget: MindBudget) -> Result<Self, HostError> {
        save::from_json(json, budget)
    }
    /// Executes one JSON command and always returns a JSON object.
    pub fn handle(&mut self, command_json: &str) -> String {
        commands::handle(self, command_json)
    }
    fn emit_fillers(&mut self, now: f64) {
        let ids: Vec<_> = self
            .request_meta
            .iter()
            .filter_map(|(id, meta)| {
                let npc = self.requests.get(id)?;
                let spec = self.fillers.get(npc)?;
                (meta.kind == RequestKind::Player
                    && !meta.filler_sent
                    && now - meta.issued_at >= spec.after_secs
                    && !spec.lines.is_empty())
                .then_some(*id)
            })
            .collect();
        for id in ids {
            let Some(npc) = self.requests.get(&id).cloned() else {
                continue;
            };
            let Some(spec) = self.fillers.get(&npc) else {
                continue;
            };
            let count = self.filler_counts.entry(npc.clone()).or_default();
            let text = spec.lines[*count % spec.lines.len()].clone();
            if let Some(data) = self.npcs.get_mut(&npc) {
                data.events.push(SessionEvent::Filler { text });
            }
            *count += 1;
            if let Some(meta) = self.request_meta.get_mut(&id) {
                meta.filler_sent = true;
            }
        }
    }
}

fn record_player_line(
    data: &mut NpcData,
    text: &str,
    now: f64,
    embedder: &dyn crate::Embedder,
) -> Result<(), HostError> {
    data.session
        .player_says(text, now)
        .map_err(|error| HostError::BadCommand(error.to_string()))?;
    data.memory.add(
        MemoryDraft::new(
            MemoryKind::Dialogue,
            format!("The player said: \"{text}\""),
            0.3,
        ),
        now,
        embedder,
    );
    Ok(())
}
