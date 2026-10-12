//! What a provider may return: a bounded list of typed commands, never code.

use serde::{Deserialize, Serialize};
use ve_graph::{Command, Diagnostic, MAX_BATCH_BYTES, MAX_COMMANDS};

/// A provider's proposal. Unknown fields are refused, so there is nowhere to put a script.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Patch {
    /// One line for the history list.
    pub summary: String,
    /// The document revision the provider based this on.
    pub base_revision: u64,
    /// The edits, applied atomically through the normal command path.
    pub commands: Vec<Command>,
}

fn invalid(message: impl Into<String>) -> Box<Diagnostic> {
    Box::new(Diagnostic::error("provider.invalid_patch", message))
}

impl Patch {
    /// Reads and bounds a provider reply.
    ///
    /// # Errors
    /// `provider.invalid_patch` for anything that is not exactly one patch: prose, code, extra
    /// fields, too many commands, or an oversized reply.
    pub fn from_reply(text: &str) -> Result<Self, Box<Diagnostic>> {
        if text.len() > MAX_BATCH_BYTES {
            return Err(invalid("the reply is larger than a batch may be"));
        }
        let patch: Self = serde_json::from_str(text.trim())
            .map_err(|e| invalid(format!("the reply is not a patch: {e}")))?;
        if patch.commands.is_empty() || patch.commands.len() > MAX_COMMANDS {
            return Err(invalid(format!("a patch has 1 to {MAX_COMMANDS} commands")));
        }
        if patch.summary.len() > 200 {
            return Err(invalid("the summary is longer than 200 bytes"));
        }
        Ok(patch)
    }
}
