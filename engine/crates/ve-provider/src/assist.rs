//! Asking a provider and applying its patch, with at most one repair attempt.

use crate::{Patch, Provider, ProviderError, Request};
use ve_graph::{CommandBatch, Diagnostic, Document, Outcome, Session, TransactionId};

/// What the host exposes to the assistant flow: the current document and a way to commit a batch.
/// The host may evaluate the candidate first, so geometry constraints can refuse an edit.
pub trait Authoring {
    /// The committed document.
    fn document(&self) -> &Document;

    /// Applies a batch atomically and reports what happened.
    fn apply(&mut self, batch: &CommandBatch) -> Outcome;
}

impl Authoring for Session {
    fn document(&self) -> &Document {
        Session::document(self)
    }

    fn apply(&mut self, batch: &CommandBatch) -> Outcome {
        Session::apply(self, batch)
    }
}

/// A committed assistant edit.
#[derive(Clone, Debug, PartialEq)]
pub struct Assisted {
    /// The patch's summary.
    pub summary: String,
    /// How many times the provider was asked (1, or 2 after a repair).
    pub attempts: u8,
    /// Revision after the commit.
    pub revision: u64,
}

fn failure(code: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(code, message)
}

/// Asks `provider` and applies its patch. A conflict or a refusal is fed back once; the second
/// failure is reported and the document stays as it was.
///
/// # Errors
/// `provider.unsupported`, `provider.failed`, `provider.invalid_patch` (no repair: nothing was
/// applied), `revision.conflict`, or the diagnostics that refused the batch.
pub fn assist(
    provider: &mut dyn Provider,
    host: &mut dyn Authoring,
    prompt: &str,
    key: &str,
) -> Result<Assisted, Vec<Diagnostic>> {
    let mut feedback = Vec::new();
    for attempt in 1..=2_u8 {
        let request = Request {
            prompt: prompt.to_owned(),
            document: host.document().clone(),
            revision: host.document().revision,
            feedback: feedback.clone(),
        };
        let text = provider.propose(&request).map_err(|e| match e {
            ProviderError::Unsupported(m) => vec![failure("provider.unsupported", m)],
            ProviderError::Failed(m) => vec![failure("provider.failed", m)],
        })?;
        let patch = Patch::from_reply(&text).map_err(|d| vec![*d])?;
        let transaction = TransactionId::parse(&format!("t_{key}{attempt}"))
            .map_err(|e| vec![failure("parse.syntax", e.to_string())])?;
        let batch = CommandBatch {
            doc: host.document().id.clone(),
            base_revision: patch.base_revision,
            transaction,
            label: Some(patch.summary.clone()),
            commands: patch.commands,
        };
        match host.apply(&batch) {
            Outcome::Committed { revision, .. } | Outcome::AlreadyApplied { revision } => {
                return Ok(Assisted {
                    summary: patch.summary,
                    attempts: attempt,
                    revision,
                });
            }
            Outcome::Rejected { diagnostics } => feedback = diagnostics,
            Outcome::Conflict {
                base_revision,
                current_revision,
                ..
            } => {
                feedback = vec![failure(
                    "revision.conflict",
                    format!(
                        "the patch was based on revision {base_revision}, the document is at {current_revision}"
                    ),
                )];
            }
        }
    }
    Err(feedback)
}
