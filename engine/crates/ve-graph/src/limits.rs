//! Hard limits checked before work begins (see `docs/modeling/limits.md`).

/// Authored nodes in one document.
pub const MAX_NODES: usize = 5_000;

/// Commands in one batch.
pub const MAX_COMMANDS: usize = 200;

/// Bytes in one serialised batch.
pub const MAX_BATCH_BYTES: usize = 256 * 1024;

/// Bytes in a serialised document.
pub const MAX_DOCUMENT_BYTES: usize = 2 * 1024 * 1024;

/// Transactions kept for undo.
pub const MAX_HISTORY: usize = 200;

/// Serialised bytes kept for undo across all transactions.
pub const MAX_HISTORY_BYTES: usize = 16 * 1024 * 1024;

/// Inputs on a variadic port.
pub(crate) const MAX_VARIADIC: usize = 64;

/// Values in a list parameter.
pub(crate) const MAX_LIST_VALUES: usize = 4_096;

/// Bytes in a text parameter or label.
pub(crate) const MAX_STRING_BYTES: usize = 256;

/// Transaction keys remembered for idempotency.
pub(crate) const MAX_SEEN_TRANSACTIONS: usize = 1_024;

/// Revisions remembered for conflict reports.
pub(crate) const MAX_REVISION_LOG: usize = 1_024;

/// Named outputs in one document.
pub(crate) const MAX_OUTPUTS: usize = 64;

/// Material definitions in one document.
pub(crate) const MAX_MATERIALS: usize = 256;

/// Entries in a material definition.
pub(crate) const MAX_MATERIAL_ENTRIES: usize = 64;

/// Largest editor-layer coordinate magnitude.
pub(crate) const MAX_EDITOR_COORD: f64 = 1.0e6;

/// Group definitions in one document.
pub(crate) const MAX_GROUPS: usize = 256;

/// Exposed parameters of one group.
pub(crate) const MAX_GROUP_PARAMS: usize = 64;

/// How deep groups may nest (an instance inside a group inside a group...).
pub const MAX_GROUP_DEPTH: usize = 8;

/// Nodes one evaluation may expand to once groups are unfolded.
pub const MAX_EXPANDED_NODES: usize = 20_000;

/// A count as a real number for a diagnostic's `limit` and `observed` fields.
#[expect(clippy::cast_precision_loss, reason = "counts are far below 2^53")]
pub(crate) fn count(n: usize) -> f64 {
    n as f64
}
