//! JavaScript exports for the document API.

use wasm_bindgen::prelude::wasm_bindgen;

use crate::document;

/// Opens a saved document or creates an empty one with the requested seed.
#[wasm_bindgen]
#[must_use]
pub fn doc_open(name: &str, seed: u32) -> String {
    document::open(name, u64::from(seed))
}

/// Starts an empty document called `name`.
#[wasm_bindgen]
#[must_use]
pub fn doc_reset(name: &str, seed: u32) -> String {
    document::reset(name, u64::from(seed))
}

/// Returns the current document and session state.
#[wasm_bindgen]
#[must_use]
pub fn doc_state() -> String {
    document::state()
}

/// Returns a retained revision document without undoing or redoing any transaction.
#[wasm_bindgen]
#[must_use]
pub fn doc_at_revision(revision: f64) -> String {
    if !revision.is_finite()
        || revision < 0.0
        || revision.fract() != 0.0
        || revision > 9_007_199_254_740_991.0
    {
        return serde_json::json!({ "error": "since must be a non-negative safe integer revision number" }).to_string();
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "validated non-negative safe integer"
    )]
    document::at_revision(revision as u64)
}

/// Applies a typed command batch and returns its outcome.
#[wasm_bindgen]
#[must_use]
pub fn doc_apply(batch_json: &str) -> String {
    document::apply(batch_json)
}

/// Undoes the last committed transaction.
#[wasm_bindgen]
#[must_use]
pub fn doc_undo() -> String {
    document::step(true)
}

/// Redoes the last undone transaction.
#[wasm_bindgen]
#[must_use]
pub fn doc_redo() -> String {
    document::step(false)
}

/// Saves the document to local storage.
#[wasm_bindgen]
#[must_use]
pub fn doc_save() -> String {
    document::save()
}

/// Returns the operator specifications known to the session.
#[wasm_bindgen]
#[must_use]
pub fn doc_operators() -> String {
    document::operators()
}

/// Parses document text into commands for the open document.
#[wasm_bindgen]
#[must_use]
pub fn doc_parse_text(text: &str) -> String {
    document::text::parse(text)
}

/// Makes graph text for a creature from a JSON request (`{"plan": "biped", "sliders": {...}}` or `{"creature": {...}}`).
/// Returns `{"text": ...}` or `{"error": ...}`.
#[wasm_bindgen]
#[must_use]
pub fn creature_make(request: &str) -> String {
    match ve_modeling::creature::make_from_json(request) {
        Ok(text) => serde_json::json!({ "text": text }).to_string(),
        Err(error) => serde_json::json!({ "error": error.to_string() }).to_string(),
    }
}

/// Parses standalone document text.
#[wasm_bindgen]
#[must_use]
pub fn doc_parse_text_standalone(text: &str) -> String {
    document::text::parse_standalone(text)
}

/// Prints the working document as canonical text.
#[wasm_bindgen]
#[must_use]
pub fn doc_text() -> String {
    document::text::print()
}

/// Returns the Rust-owned candidate document and its dependency hash.
#[wasm_bindgen]
#[must_use]
pub fn doc_candidate_state() -> String {
    document::staging::state()
}

/// Stages a command batch until its geometry succeeds.
#[wasm_bindgen]
#[must_use]
pub fn doc_stage(batch: &str) -> String {
    document::staging::apply(batch)
}

/// Stages an undo until evaluation succeeds.
#[wasm_bindgen]
#[must_use]
pub fn doc_stage_undo() -> String {
    document::staging::step(true)
}

/// Stages a redo until evaluation succeeds.
#[wasm_bindgen]
#[must_use]
pub fn doc_stage_redo() -> String {
    document::staging::step(false)
}
