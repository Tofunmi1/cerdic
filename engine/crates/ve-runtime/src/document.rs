//! The authoring document as the page sees it: one session, saved to the browser's local storage.
//!
//! Every function takes and returns JSON text, so the page deals in plain data and no engine type
//! crosses the boundary. Failures are values (`{ "rejected": ... }`, `{ "error": ... }`), never
//! exceptions. Nothing here evaluates geometry; this is the editing core of the pipeline.

use std::cell::RefCell;

use serde_json::{Value, json};
use ve_graph::{
    Backend, BackendError, Diagnostic, DocStore, Document, Migrator, Outcome, Session,
    builtin_registry,
};
use wasm_bindgen::JsCast;
pub(crate) mod staging;

/// The browser's `localStorage` as a [`Backend`].
struct LocalBackend;

fn storage() -> Result<web_sys::Storage, BackendError> {
    web_sys::window()
        .ok_or_else(|| BackendError::Failed("there is no window".to_owned()))?
        .local_storage()
        .map_err(|_| BackendError::Failed("local storage is blocked".to_owned()))?
        .ok_or_else(|| BackendError::Failed("local storage is unavailable".to_owned()))
}

fn is_quota(error: &wasm_bindgen::JsValue) -> bool {
    error
        .dyn_ref::<web_sys::DomException>()
        .is_some_and(|e| e.name() == "QuotaExceededError")
}

impl Backend for LocalBackend {
    fn get(&self, key: &str) -> Result<Option<String>, BackendError> {
        storage()?
            .get_item(key)
            .map_err(|_| BackendError::Failed("cannot read local storage".to_owned()))
    }

    fn set(&mut self, key: &str, value: &str) -> Result<(), BackendError> {
        storage()?.set_item(key, value).map_err(|e| {
            if is_quota(&e) {
                BackendError::QuotaExceeded {
                    needed: value.len(),
                }
            } else {
                BackendError::Failed("cannot write local storage".to_owned())
            }
        })
    }
}

struct Host {
    session: Session,
    store: DocStore<LocalBackend>,
    /// Set when the document must not be saved over what is in storage.
    read_only: bool,
    issues: Vec<Diagnostic>,
    saved_generation: Option<u64>,
    pending: Option<Session>,
    expected: Option<ve_modeling::JobToken>,
}

thread_local! {
    static HOST: RefCell<Option<Host>> = const { RefCell::new(None) };
}

fn fresh(name: &str, seed: u64) -> Host {
    Host {
        session: Session::with_builtin(seed),
        store: DocStore::new(LocalBackend, name),
        read_only: false,
        issues: Vec::new(),
        saved_generation: None,
        pending: None,
        expected: None,
    }
}

fn with_host<T>(f: impl FnOnce(&mut Host) -> T) -> T {
    HOST.with(|cell| {
        let mut slot = cell.borrow_mut();
        f(slot.get_or_insert_with(|| fresh("default", 1)))
    })
}

fn read_only_error() -> Diagnostic {
    Diagnostic::error(
        "store.read_only",
        "this document is read-only: it holds data this build cannot edit, or it failed to load",
    )
}

fn state_of(host: &Host) -> Value {
    let doc = host.session.document();
    json!({
        "id": doc.id,
        "revision": doc.revision,
        "nodes": doc.nodes.len(),
        "can_undo": host.session.can_undo(),
        "can_redo": host.session.can_redo(),
        "read_only": host.read_only,
        "saved_generation": host.saved_generation,
        "issues": host.issues,
        "document": doc,
        "pending_document": host.pending.as_ref().map(Session::document),
        "evaluation_hash": ve_modeling::hash(doc),
    })
}

/// Opens the document called `name` from local storage, or starts an empty one with `seed`. A
/// document that fails to load, or holds operators this build lacks, opens read-only with the
/// reasons in `issues`; storage is never modified by opening.
pub(crate) fn open(name: &str, seed: u64) -> String {
    let mut host = fresh(name, seed);
    match host.store.load(&builtin_registry(), &Migrator::builtin()) {
        Ok(Some(loaded)) => {
            host.session = Session::new(builtin_registry(), loaded.document);
            host.read_only = loaded.read_only;
            host.issues = loaded.issues;
            host.saved_generation = Some(loaded.generation);
        }
        Ok(None) => {}
        Err(problem) => {
            host.read_only = true;
            host.issues = vec![*problem];
        }
    }
    let text = state_of(&host).to_string();
    HOST.with(|cell| *cell.borrow_mut() = Some(host));
    text
}

/// Replaces the session with an empty document (not saved until `save`).
pub(crate) fn reset(name: &str, seed: u64) -> String {
    let host = fresh(name, seed);
    let text = state_of(&host).to_string();
    HOST.with(|cell| *cell.borrow_mut() = Some(host));
    text
}

/// The current document and session state.
pub(crate) fn state() -> String {
    with_host(|host| state_of(host).to_string())
}

/// Applies a command batch given as JSON; returns the [`Outcome`] as JSON.
pub(crate) fn apply(batch_json: &str) -> String {
    with_host(|host| {
        let outcome = if host.read_only {
            Outcome::Rejected {
                diagnostics: vec![read_only_error()],
            }
        } else {
            host.session.apply_json(batch_json)
        };
        if matches!(outcome, Outcome::Committed { .. }) {
            host.pending = None;
            host.expected = None;
        }
        serde_json::to_string(&outcome).unwrap_or_default()
    })
}

/// Undoes or redoes one transaction; returns `{ "revision", "changed" }` or `null`.
pub(crate) fn step(undo: bool) -> String {
    with_host(|host| {
        let moved = if host.read_only {
            None
        } else if undo {
            host.session.undo()
        } else {
            host.session.redo()
        };
        if moved.is_some() {
            host.pending = None;
            host.expected = None;
        }
        moved.map_or_else(
            || "null".to_owned(),
            |m| json!({ "revision": m.revision, "changed": m.changed }).to_string(),
        )
    })
}

/// Saves the document; returns `{ "generation" }` or `{ "error": <diagnostic> }`.
pub(crate) fn save() -> String {
    with_host(|host| {
        if host.pending.is_some() {
            return json!({"error":Diagnostic::error("eval.pending", "finish evaluation before saving the candidate")}).to_string();
        }
        if host.read_only {
            return json!({ "error": read_only_error() }).to_string();
        }
        let doc: Document = host.session.document().clone();
        match host.store.save(&doc) {
            Ok(generation) => {
                host.saved_generation = Some(generation);
                json!({ "generation": generation }).to_string()
            }
            Err(problem) => json!({ "error": *problem }).to_string(),
        }
    })
}

/// Every operator the session knows, as JSON (ports, parameters, bounds, readiness).
pub(crate) fn operators() -> String {
    with_host(|host| {
        let specs: Vec<_> = host.session.registry().iter().collect();
        serde_json::to_string(&specs).unwrap_or_default()
    })
}
