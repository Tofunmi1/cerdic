//! Identity shared across the evaluation and projection boundaries.
use crate::Quality;
use serde::{Deserialize, Serialize};
use ve_graph::Document;

/// Complete identity of one immutable evaluation request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobToken {
    /// Document identity.
    pub document: String,
    /// Working candidate revision.
    pub revision: u64,
    /// Supervisor request sequence.
    pub request: u64,
    /// Worker incarnation; increases on restart.
    pub generation: u32,
    /// Evaluation quality.
    pub quality: Quality,
    /// Dependency digest of the authored content.
    pub hash: String,
}

impl JobToken {
    /// Whether this request describes exactly this document.
    #[must_use]
    pub fn matches(&self, doc: &Document) -> bool {
        self.document == doc.id.as_str() && self.revision == doc.revision && self.hash == hash(doc)
    }
}

/// Stable text form of the authored evaluation hash.
#[must_use]
pub fn hash(doc: &Document) -> String {
    let values = doc.eval_hash().0;
    format!("{:016x}{:016x}", values[0], values[1])
}
