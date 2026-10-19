//! Candidate documents remain owned by Rust until their matching projection is ready.
use super::{read_only_error, with_host};
use serde_json::json;
use ve_graph::Outcome;
use ve_modeling::{JobToken, hash};

pub(crate) fn state() -> String {
    with_host(|host| {
        let doc = host.pending.as_ref().unwrap_or(&host.session).document();
        json!({"document": doc, "hash": hash(doc), "pending": host.pending.is_some()}).to_string()
    })
}

pub(crate) fn apply(text: &str) -> String {
    with_host(|host| {
        if host.read_only {
            return json!({"rejected":{"diagnostics":[read_only_error()]}}).to_string();
        }
        let mut candidate = host.pending.as_ref().unwrap_or(&host.session).clone();
        let outcome = candidate.apply_json(text);
        if matches!(outcome, Outcome::Committed { .. }) {
            host.pending = Some(candidate);
            host.expected = None;
            json!({"pending":outcome}).to_string()
        } else {
            serde_json::to_string(&outcome).unwrap_or_default()
        }
    })
}

pub(crate) fn step(undo: bool) -> String {
    with_host(|host| {
        if host.read_only {
            return "null".to_owned();
        }
        let mut candidate = host.pending.as_ref().unwrap_or(&host.session).clone();
        let moved = if undo {
            candidate.undo()
        } else {
            candidate.redo()
        };
        if let Some(moved) = moved {
            host.pending = Some(candidate);
            host.expected = None;
            json!({"revision":moved.revision,"changed":moved.changed}).to_string()
        } else {
            "null".to_owned()
        }
    })
}

pub(crate) fn bind(text: &str) -> bool {
    let Ok(token) = serde_json::from_str::<JobToken>(text) else {
        return false;
    };
    with_host(|host| {
        if !token.matches(host.pending.as_ref().unwrap_or(&host.session).document()) {
            return false;
        }
        host.expected = Some(token);
        true
    })
}

pub(crate) fn current(token: &JobToken) -> bool {
    with_host(|host| {
        let doc = host.pending.as_ref().unwrap_or(&host.session).document();
        host.expected.as_ref() == Some(token)
            && token.document == doc.id.as_str()
            && token.revision == doc.revision
    })
}

pub(crate) fn accept(token: &JobToken) -> bool {
    with_host(|host| {
        if host.expected.as_ref() != Some(token)
            || !token.matches(host.pending.as_ref().unwrap_or(&host.session).document())
        {
            return false;
        }
        if let Some(candidate) = host.pending.take() {
            host.session = candidate;
        }
        host.issues.clear();
        true
    })
}

pub(crate) fn reject(text: &str, code: &str, message: &str) -> bool {
    let Ok(token) = serde_json::from_str::<JobToken>(text) else {
        return false;
    };
    with_host(|host| {
        if host.expected.as_ref() != Some(&token) {
            return false;
        }
        if token.quality == ve_modeling::Quality::Preview
            && code == "budget.triangles"
            && host
                .pending
                .as_ref()
                .is_some_and(|candidate| token.matches(candidate.document()))
        {
            // This is the one preview refusal that can be resolved by a higher quality. Keep the
            // staged edit available for its automatic Final request; Final either commits it or
            // rejects the transaction normally.
            host.expected = None;
            host.issues.clear();
            return true;
        }
        host.pending = None;
        host.expected = None;
        host.issues = vec![ve_graph::Diagnostic::error(code, message)];
        true
    })
}
