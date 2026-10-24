//! Applies completed mind deliveries to dialogue and memory.
use super::NpcHost;
use crate::{ArgValue, Delivery, MemoryDraft, MemoryKind};

pub(super) fn guard(host: &mut NpcHost, npc: &str, delivery: &mut Delivery) {
    let Some(data) = host.npcs.get_mut(npc) else {
        return;
    };
    let trust = data.memory.relationship("player").trust;
    for intent in &mut delivery.intents {
        if intent.name != "say" {
            continue;
        }
        let Some(ArgValue::Text(text)) = intent.args.get_mut("text") else {
            continue;
        };
        let lowered = text.to_lowercase();
        let marker = data
            .secrets
            .iter()
            .filter(|s| trust < s.min_trust)
            .flat_map(|s| s.markers.iter())
            .find(|m| !m.is_empty() && lowered.contains(&m.to_lowercase()))
            .cloned();
        let Some(marker) = marker else {
            continue;
        };
        let raw: String = text.chars().take(200).collect();
        *text = if data.deflections.is_empty() {
            "Let's talk about something else.".into()
        } else {
            data.deflections[(data.leaks_blocked as usize) % data.deflections.len()].clone()
        };
        delivery.rejected.push(crate::RejectedIntent {
            raw,
            reason: format!("leak guard: marker '{marker}'"),
        });
        data.leaks_blocked = data.leaks_blocked.saturating_add(1);
    }
}

pub(super) fn apply(
    host: &mut NpcHost,
    npc: &str,
    delivery: &Delivery,
    now: f64,
    conversation: bool,
) {
    let Some(data) = host.npcs.get_mut(npc) else {
        return;
    };
    if conversation {
        let _ = data.session.apply_delivery(delivery, now);
    }
    let line = delivery.intents.iter().find_map(|intent| {
        if intent.name != "say" {
            return None;
        }
        match intent.args.get("text") {
            Some(ArgValue::Text(text)) => Some(text.clone()),
            _ => None,
        }
    });
    for intent in &delivery.intents {
        match intent.name.as_str() {
            "remember" => remember(data, intent, now, &host.embedder),
            "reflect" => reflect(data, intent, now, &host.embedder),
            "set_fact" => set_fact(data, intent, now),
            "adjust_relationship" => adjust_relationship(data, intent),
            _ => {}
        }
    }
    if let Some(text) = line.filter(|_| conversation && !delivery.fallback) {
        data.memory.add(
            MemoryDraft::new(
                MemoryKind::Dialogue,
                format!("{} said: \"{text}\"", data.spec.name),
                0.3,
            ),
            now,
            &host.embedder,
        );
    }
    if conversation {
        data.events.extend(data.session.drain_events());
    }
}

fn reflect(
    data: &mut super::NpcData,
    intent: &crate::Intent,
    now: f64,
    embedder: &dyn crate::Embedder,
) {
    if let Some(ArgValue::Text(text)) = intent.args.get("text") {
        let mut draft = MemoryDraft::new(MemoryKind::Reflection, text, 0.8);
        draft.source = crate::Source::Reflected;
        let id = data.memory.add(draft, now, embedder);
        data.memory.pin(id);
    }
}

fn remember(
    data: &mut super::NpcData,
    intent: &crate::Intent,
    now: f64,
    embedder: &dyn crate::Embedder,
) {
    if let (Some(ArgValue::Text(text)), Some(ArgValue::Number(importance))) =
        (intent.args.get("text"), intent.args.get("importance"))
    {
        data.memory.add(
            MemoryDraft::new(MemoryKind::Event, text, as_f32(*importance)),
            now,
            embedder,
        );
    }
}

fn set_fact(data: &mut super::NpcData, intent: &crate::Intent, now: f64) {
    if let (Some(ArgValue::Text(key)), Some(ArgValue::Text(value))) =
        (intent.args.get("key"), intent.args.get("value"))
    {
        data.memory.set_fact(key, value, 0.8, now);
    }
}

fn adjust_relationship(data: &mut super::NpcData, intent: &crate::Intent) {
    if let (
        Some(ArgValue::Text(other)),
        Some(ArgValue::Number(affinity)),
        Some(ArgValue::Number(trust)),
    ) = (
        intent.args.get("other"),
        intent.args.get("affinity"),
        intent.args.get("trust"),
    ) {
        data.memory
            .adjust_relationship(other, as_f32(*affinity), as_f32(*trust), 0.0);
    }
}

fn as_f32(value: f64) -> f32 {
    value
        .clamp(-f64::from(f32::MAX), f64::from(f32::MAX))
        .to_string()
        .parse::<f32>()
        .unwrap_or(0.0)
}
