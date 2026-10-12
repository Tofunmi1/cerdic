//! The text form of the document for the page: read text into commands, print the document.
use super::with_host;
use serde_json::json;
use ve_graph::{builtin_registry, parse_text, to_text};

/// Text to `{ commands, spans }` (the page wraps them in a batch), or `{ diagnostics }`.
pub(crate) fn parse(text: &str) -> String {
    match parse_text(&builtin_registry(), text) {
        Ok(batch) => json!({"commands": batch.commands, "spans": batch.spans}),
        Err(diagnostics) => json!({"diagnostics": diagnostics}),
    }
    .to_string()
}

/// The working candidate (or committed document) as canonical text: `{ text }` or `{ error }`.
pub(crate) fn print() -> String {
    with_host(|host| {
        let doc = host.pending.as_ref().unwrap_or(&host.session).document();
        match to_text(host.session.registry(), doc) {
            Ok(text) => json!({"text": text}),
            Err(error) => json!({"error": error}),
        }
        .to_string()
    })
}
