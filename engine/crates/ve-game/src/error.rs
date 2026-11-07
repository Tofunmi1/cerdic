//! Structured parser diagnostics and runtime failures.
/// A location-aware definition diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// One-based source line.
    pub line: usize,
    /// One-based source column.
    pub column: usize,
    /// Human-readable problem.
    pub message: String,
    /// A concrete repair hint.
    pub suggestion: String,
}
impl Diagnostic {
    pub(crate) fn new(
        line: usize,
        column: usize,
        message: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            line,
            column,
            message: message.into(),
            suggestion: suggestion.into(),
        }
    }
}
/// Runtime error that prevents a deterministic step or restore.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum GameError {
    /// Entity budget reached.
    #[error("entity budget exceeded ({0})")]
    EntityBudget(usize),
    /// Action budget reached.
    #[error("action budget exceeded ({0})")]
    ActionBudget(usize),
    /// Snapshot does not match the world's definition.
    #[error("snapshot belongs to a different game definition")]
    SnapshotMismatch,
    /// Invalid input axis or numeric value.
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// Event cascade exceeded the permitted depth.
    #[error("event chain depth exceeded ({0})")]
    ChainDepth(usize),
    /// A script used up its work budget or could not run.
    #[error("script stopped: {0}")]
    Script(String),
}
