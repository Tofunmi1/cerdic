//! Reflection prompt construction and scheduling.
use super::{HostError, NpcHost, RequestKind, RequestMeta};
use crate::{
    ArgType, IntentSpec, MemoryKind, PromptBudget, PromptSections, Reason, Vocabulary,
    assemble_prompt,
};
use std::fmt::Write as _;

pub(super) fn maybe_reflect(host: &mut NpcHost, npc: &str, now: f64) -> Result<bool, HostError> {
    let data = host
        .npcs
        .get(npc)
        .ok_or_else(|| HostError::UnknownNpc(npc.into()))?;
    if host.reflection_outstanding.contains(npc)
        || !data.memory.reflection_due(host.reflection_threshold)
    {
        return Ok(false);
    }
    let memories = data
        .memory
        .recent(usize::MAX)
        .into_iter()
        .filter(|m| m.kind != MemoryKind::Reflection)
        .take(15)
        .map(|m| format!("({}) {}", super::age::format_age(now - m.time), m.text))
        .collect();
    let mut rules = data.spec.rules.clone();
    if !data.examples.is_empty() {
        rules.push_str("\n\n## Examples\n");
        for (q, a) in &data.examples {
            let _ = writeln!(rules, "player: {q}\n{}: {a}", data.spec.name);
        }
    }
    let sections = PromptSections { persona: data.spec.persona.clone(), rules, world: data.spec.world_facts.clone(), memories,
        situation: "Look over what has happened recently and write one or two lasting conclusions about the player or the village that will help you in future conversations.".into(), ..PromptSections::default() };
    let mut vocabulary = Vocabulary::new();
    let _ = vocabulary.register(
        IntentSpec::new("reflect", "Record a lasting conclusion.")
            .arg("text", ArgType::Text { max_len: 300 }),
    );
    let mut prompt = assemble_prompt(&sections, &vocabulary, &PromptBudget { max_chars: 12_000 });
    prompt.text = prompt.text.replace(
        "\"name\":\"say\",\"args\":{\"text\":\"Hello there!\"}",
        "\"name\":\"reflect\",\"args\":{\"text\":\"The player is generous.\"}",
    );
    match host.mind.ask(npc, Reason::Reflection, &prompt.text, now) {
        crate::Ask::Sent(id) | crate::Ask::Queued(id) => {
            host.requests.insert(id, npc.into());
            host.attempts.insert(id, 0);
            host.request_meta.insert(
                id,
                RequestMeta {
                    issued_at: now,
                    kind: RequestKind::Reflection,
                    filler_sent: false,
                    prompt: prompt.text,
                },
            );
            host.reflection_outstanding.insert(npc.into());
            Ok(true)
        }
        crate::Ask::Refused(_) => Ok(false),
    }
}
