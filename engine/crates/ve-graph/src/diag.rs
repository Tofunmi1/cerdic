//! Diagnostics: one shape for every problem (see `docs/modeling/diagnostics.md`).

use serde::{Deserialize, Serialize};

use crate::id::NodeId;
use crate::value::ParamIssue;

/// How serious a diagnostic is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Blocks a commit.
    Error,
    /// Reported, does not block.
    Warning,
    /// Progress or a no-op.
    Info,
}

/// A problem found while validating, planning or evaluating.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Stable dotted code, for example `graph.cycle`.
    pub code: String,
    /// How serious it is.
    pub severity: Severity,
    /// What happened, in words (may change between versions; match on `code`).
    pub message: String,
    /// Position of the command in its batch, when the problem came from one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<usize>,
    /// The smallest responsible node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<NodeId>,
    /// The port involved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<String>,
    /// The parameter involved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param: Option<String>,
    /// Other nodes that took part (the members of a cycle, the source of a bad link).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub related: Vec<NodeId>,
    /// The limit that was hit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<f64>,
    /// The value that hit it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed: Option<f64>,
    /// What to try instead.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl Diagnostic {
    /// An error with the given code and message.
    #[must_use]
    pub fn error(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            severity: Severity::Error,
            message: message.into(),
            command: None,
            node: None,
            port: None,
            param: None,
            related: Vec::new(),
            limit: None,
            observed: None,
            suggestion: None,
        }
    }

    /// An error built from a refused parameter value.
    #[must_use]
    pub fn from_issue(issue: ParamIssue, node: Option<&NodeId>, param: &str) -> Self {
        Self {
            node: node.cloned(),
            param: Some(param.to_owned()),
            limit: issue.limit,
            observed: issue.observed,
            ..Self::error(issue.code, issue.message)
        }
    }

    /// Names the node.
    #[must_use]
    pub fn at_node(mut self, node: &NodeId) -> Self {
        self.node = Some(node.clone());
        self
    }

    /// Names the port.
    #[must_use]
    pub fn at_port(mut self, port: &str) -> Self {
        self.port = Some(port.to_owned());
        self
    }

    /// Lists related nodes.
    #[must_use]
    pub fn with_related(mut self, related: Vec<NodeId>) -> Self {
        self.related = related;
        self
    }

    /// Records a limit and the value that exceeded it.
    #[must_use]
    pub fn with_limit(mut self, limit: f64, observed: f64) -> Self {
        self.limit = Some(limit);
        self.observed = Some(observed);
        self
    }

    /// Adds a suggestion.
    #[must_use]
    pub fn suggest(mut self, text: impl Into<String>) -> Self {
        self.suggestion = Some(text.into());
        self
    }

    /// Records which command in the batch caused it.
    #[must_use]
    pub fn at_command(mut self, index: usize) -> Self {
        self.command = Some(index);
        self
    }
}
