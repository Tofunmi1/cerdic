//! The authoring session: the committed document, its history and the rules for applying batches.
//!
//! A batch is applied to a candidate copy; the committed document changes only on success and a
//! matching revision. Undo and redo move along recorded transactions and advance the revision (so
//! a batch based on a pre-undo revision conflicts), but never rewind the ID counter (so an ID is
//! never reused).

use std::collections::VecDeque;

use crate::apply::apply_batch;
use crate::builtin::builtin_registry;
use crate::command::{CommandBatch, Outcome};
use crate::diag::Diagnostic;
use crate::doc::Document;
use crate::id::{NodeId, TransactionId};
use crate::limits::{
    MAX_COMMANDS, MAX_HISTORY, MAX_HISTORY_BYTES, MAX_REVISION_LOG, MAX_SEEN_TRANSACTIONS,
};
use crate::spec::OperatorRegistry;

/// The longest label kept for a history entry, in characters.
const MAX_LABEL: usize = 80;

#[derive(Clone)]
struct UndoEntry {
    before: Document,
    changed: Vec<NodeId>,
    label: String,
    bytes: usize,
}

#[derive(Clone)]
struct RedoEntry {
    after: Document,
    changed: Vec<NodeId>,
    label: String,
}

/// One transaction in the history list.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct HistoryItem {
    /// What the person or tool said the transaction was for ("Edit" when it said nothing).
    pub label: String,
    /// Nodes the transaction touched.
    pub changed: Vec<NodeId>,
}

/// What undo and redo can reach, for a history panel.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct History {
    /// Transactions that can be undone, oldest first (undo takes the last).
    pub undo: Vec<HistoryItem>,
    /// Transactions that can be redone, the next one first.
    pub redo: Vec<HistoryItem>,
}

fn history_label(label: Option<&str>) -> String {
    let text = label.map_or("", str::trim);
    if text.is_empty() {
        "Edit".to_owned()
    } else {
        text.chars().take(MAX_LABEL).collect()
    }
}

/// Where undo or redo moved the document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Moved {
    /// The new revision.
    pub revision: u64,
    /// Nodes the moved transaction touched.
    pub changed: Vec<NodeId>,
}

/// A document under edit.
#[derive(Clone)]
pub struct Session {
    registry: OperatorRegistry,
    doc: Document,
    undo: VecDeque<UndoEntry>,
    undo_bytes: usize,
    redo: Vec<RedoEntry>,
    seen: VecDeque<(TransactionId, u64)>,
    log: VecDeque<(u64, Vec<NodeId>)>,
}

impl Session {
    /// Starts a session on `doc` with the given operators.
    #[must_use]
    pub fn new(registry: OperatorRegistry, doc: Document) -> Self {
        Self {
            registry,
            doc,
            undo: VecDeque::new(),
            undo_bytes: 0,
            redo: Vec::new(),
            seen: VecDeque::new(),
            log: VecDeque::new(),
        }
    }

    /// A session on an empty document with the built-in operators.
    #[must_use]
    pub fn with_builtin(seed: u64) -> Self {
        Self::new(builtin_registry(), Document::new(seed))
    }

    /// The committed document.
    #[must_use]
    pub fn document(&self) -> &Document {
        &self.doc
    }

    /// The operators this session validates against.
    #[must_use]
    pub fn registry(&self) -> &OperatorRegistry {
        &self.registry
    }

    /// Whether there is anything to undo.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether there is anything to redo.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// The transactions undo and redo can reach, for a history list.
    #[must_use]
    pub fn history(&self) -> History {
        let item = |label: &String, changed: &Vec<NodeId>| HistoryItem {
            label: label.clone(),
            changed: changed.clone(),
        };
        History {
            undo: self
                .undo
                .iter()
                .map(|entry| item(&entry.label, &entry.changed))
                .collect(),
            redo: self
                .redo
                .iter()
                .rev()
                .map(|entry| item(&entry.label, &entry.changed))
                .collect(),
        }
    }

    fn rejected(code: &str, message: impl Into<String>) -> Outcome {
        Outcome::Rejected {
            diagnostics: vec![Diagnostic::error(code, message)],
        }
    }

    fn remember_revision(&mut self, changed: &[NodeId]) {
        self.log.push_back((self.doc.revision, changed.to_vec()));
        while self.log.len() > MAX_REVISION_LOG {
            self.log.pop_front();
        }
    }

    fn conflict(&self, base: u64) -> Outcome {
        let current = self.doc.revision;
        let mut changed: Vec<NodeId> = self
            .log
            .iter()
            .filter(|(rev, _)| *rev > base)
            .flat_map(|(_, ids)| ids.iter().cloned())
            .collect();
        changed.sort();
        changed.dedup();
        let complete = self
            .log
            .front()
            .is_some_and(|(first, _)| *first <= base.saturating_add(1));
        Outcome::Conflict {
            base_revision: base,
            current_revision: current,
            changed_since: changed,
            complete,
        }
    }

    /// Applies a batch. See [`Outcome`] for the four possible results.
    pub fn apply(&mut self, batch: &CommandBatch) -> Outcome {
        if batch.doc != self.doc.id {
            return Self::rejected("revision.document", "the batch is for another document");
        }
        if let Some((_, revision)) = self.seen.iter().find(|(key, _)| *key == batch.transaction) {
            return Outcome::AlreadyApplied {
                revision: *revision,
            };
        }
        if batch.base_revision != self.doc.revision {
            return self.conflict(batch.base_revision);
        }
        if batch.commands.is_empty() {
            return Self::rejected("parse.syntax", "the batch has no commands");
        }
        if batch.commands.len() > MAX_COMMANDS {
            #[expect(clippy::cast_precision_loss, reason = "counts are far below 2^53")]
            return Outcome::Rejected {
                diagnostics: vec![
                    Diagnostic::error("budget.commands", "too many commands in one batch")
                        .with_limit(MAX_COMMANDS as f64, batch.commands.len() as f64)
                        .suggest(format!(
                            "split the edit into batches of at most {MAX_COMMANDS} commands"
                        )),
                ],
            };
        }
        let applied = match apply_batch(&self.doc, &self.registry, &batch.commands) {
            Ok(applied) => applied,
            Err(diagnostics) => return Outcome::Rejected { diagnostics },
        };
        let bytes = self.doc.canonical_json().len();
        self.undo.push_back(UndoEntry {
            before: self.doc.clone(),
            changed: applied.changed.clone(),
            label: history_label(batch.label.as_deref()),
            bytes,
        });
        self.undo_bytes += bytes;
        while self.undo.len() > MAX_HISTORY || self.undo_bytes > MAX_HISTORY_BYTES {
            let Some(old) = self.undo.pop_front() else {
                break;
            };
            self.undo_bytes = self.undo_bytes.saturating_sub(old.bytes);
        }
        self.redo.clear();
        self.doc = applied.doc;
        self.doc.revision += 1;
        self.remember_revision(&applied.changed);
        self.seen
            .push_back((batch.transaction.clone(), self.doc.revision));
        while self.seen.len() > MAX_SEEN_TRANSACTIONS {
            self.seen.pop_front();
        }
        Outcome::Committed {
            revision: self.doc.revision,
            transaction: batch.transaction.clone(),
            changed: applied.changed,
        }
    }

    /// Reads a batch from JSON and applies it; unreadable or oversized input is `Rejected`.
    pub fn apply_json(&mut self, text: &str) -> Outcome {
        match CommandBatch::from_json(text) {
            Ok(batch) => self.apply(&batch),
            Err(diagnostic) => Outcome::Rejected {
                diagnostics: vec![*diagnostic],
            },
        }
    }

    fn install(&mut self, mut next: Document, changed: &[NodeId]) -> Moved {
        next.revision = self.doc.revision + 1;
        next.ids = self.doc.ids;
        self.doc = next;
        self.remember_revision(changed);
        Moved {
            revision: self.doc.revision,
            changed: changed.to_vec(),
        }
    }

    /// Reverts the most recent transaction; `None` if there is nothing to undo.
    pub fn undo(&mut self) -> Option<Moved> {
        let entry = self.undo.pop_back()?;
        self.undo_bytes = self.undo_bytes.saturating_sub(entry.bytes);
        self.redo.push(RedoEntry {
            after: self.doc.clone(),
            changed: entry.changed.clone(),
            label: entry.label,
        });
        Some(self.install(entry.before, &entry.changed))
    }

    /// Re-applies the most recently undone transaction; `None` if there is nothing to redo.
    pub fn redo(&mut self) -> Option<Moved> {
        let entry = self.redo.pop()?;
        let bytes = self.doc.canonical_json().len();
        self.undo.push_back(UndoEntry {
            before: self.doc.clone(),
            changed: entry.changed.clone(),
            label: entry.label,
            bytes,
        });
        self.undo_bytes += bytes;
        Some(self.install(entry.after, &entry.changed))
    }
}
