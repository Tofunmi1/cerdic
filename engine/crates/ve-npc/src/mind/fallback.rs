use super::{Reason, Request};
use crate::{Intent, Vocabulary, parse_reply};
use std::collections::BTreeMap;

/// Ordered canned lines, grouped by reason.
#[derive(Clone, Debug, Default)]
pub struct FallbackTable {
    pub(super) lines: Vec<(Reason, String)>,
}

impl FallbackTable {
    /// Creates an empty fallback table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a line for a reason, preserving insertion order.
    #[must_use]
    pub fn line(mut self, reason: Reason, line: impl Into<String>) -> Self {
        self.lines.push((reason, line.into()));
        self
    }
}

pub(super) fn fallback_intents(
    fallbacks: &BTreeMap<String, FallbackTable>,
    request: &Request,
    vocab: &Vocabulary,
) -> Vec<Intent> {
    let Some(table) = fallbacks.get(&request.npc) else {
        return Vec::new();
    };
    let lines: Vec<&String> = table
        .lines
        .iter()
        .filter(|(reason, _)| *reason == request.reason)
        .map(|(_, line)| line)
        .collect();
    if lines.is_empty() {
        return Vec::new();
    }
    let mut bytes = request.npc.as_bytes().to_vec();
    bytes.extend(request.id.0.to_le_bytes());
    let index = usize::try_from(super::recording::fnv(&bytes)).unwrap_or(0) % lines.len();
    let reply = format!(
        r#"{{"intents":[{{"name":"say","args":{{"text":{}}}}}]}}"#,
        serde_json::to_string(lines[index]).unwrap_or_default()
    );
    parse_reply(&reply, vocab).intents
}
