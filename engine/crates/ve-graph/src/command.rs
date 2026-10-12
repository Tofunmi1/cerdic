//! The typed command set (see `docs/modeling/commands.md`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::diag::Diagnostic;
use crate::id::{DocId, IdError, MaterialId, NodeId, TransactionId};
use crate::limits::MAX_BATCH_BYTES;
use crate::value::Value;

/// A name valid only inside one batch for a node the batch adds: `$` plus 1 to 32 characters from
/// `[A-Za-z0-9_]`. It lets later commands in the same batch refer to a node whose ID is issued when
/// the batch is applied.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Alias(String);

impl Alias {
    /// Validates and wraps `s`.
    ///
    /// # Errors
    /// [`IdError`] if `s` is not `$` followed by 1 to 32 allowed characters.
    pub fn parse(s: &str) -> Result<Self, IdError> {
        let ok = s.strip_prefix('$').is_some_and(|body| {
            (1..=32).contains(&body.len())
                && body.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        });
        if ok {
            Ok(Self(s.to_owned()))
        } else {
            Err(IdError {
                kind: "alias",
                value: s.chars().take(80).collect(),
            })
        }
    }

    /// The alias as text, including the `$`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Alias {
    type Error = IdError;
    fn try_from(s: String) -> Result<Self, IdError> {
        Self::parse(&s)
    }
}

impl From<Alias> for String {
    fn from(a: Alias) -> String {
        a.0
    }
}

/// A node named in a command: by its real ID, or by an alias the same batch defined.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum NodeRef {
    /// An existing node.
    Id(NodeId),
    /// A node added earlier in this batch.
    Alias(Alias),
}

impl TryFrom<String> for NodeRef {
    type Error = IdError;
    fn try_from(s: String) -> Result<Self, IdError> {
        if s.starts_with('$') {
            Alias::parse(&s).map(Self::Alias)
        } else {
            NodeId::parse(&s).map(Self::Id)
        }
    }
}

impl From<NodeRef> for String {
    fn from(r: NodeRef) -> String {
        match r {
            NodeRef::Id(id) => id.into(),
            NodeRef::Alias(alias) => alias.into(),
        }
    }
}

impl From<NodeId> for NodeRef {
    fn from(id: NodeId) -> Self {
        Self::Id(id)
    }
}

/// An output port named in a command.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutRef {
    /// The producing node.
    pub node: NodeRef,
    /// Its output port (always `out` for now).
    pub port: String,
}

impl OutRef {
    /// The `out` port of `node`.
    #[must_use]
    pub fn out(node: impl Into<NodeRef>) -> Self {
        Self {
            node: node.into(),
            port: crate::doc::OUT_PORT.to_owned(),
        }
    }
}

/// An input port of a node, optionally at a position in a variadic list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputRef {
    /// The consuming node.
    pub node: NodeRef,
    /// Its input port.
    pub port: String,
    /// Position in a variadic port; `None` appends on connect and removes the only link on
    /// disconnect of a single-link port.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
}

/// One edit. Batches apply all of their commands or none.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    /// Adds a node with a freshly issued ID.
    AddNode {
        /// Operator and version, `family.name@N`.
        op: String,
        /// Parameter values; missing ones take their defaults.
        #[serde(default)]
        params: BTreeMap<String, Value>,
        /// Display name.
        #[serde(default)]
        label: Option<String>,
        /// A name later commands in this batch can use for the new node.
        #[serde(default)]
        alias: Option<Alias>,
    },
    /// Removes a node.
    RemoveNode {
        /// The node.
        node: NodeRef,
        /// Also remove everything that reads it (and outputs that name it).
        #[serde(default)]
        cascade: bool,
    },
    /// Links an output to an input.
    Connect {
        /// Source.
        from: OutRef,
        /// Destination.
        to: InputRef,
    },
    /// Removes a link.
    Disconnect {
        /// The input to clear.
        to: InputRef,
    },
    /// Sets a parameter.
    SetParam {
        /// The node.
        node: NodeRef,
        /// The parameter.
        param: String,
        /// The new value.
        value: Value,
    },
    /// Sets or clears the display name.
    SetLabel {
        /// The node.
        node: NodeRef,
        /// The name, or `None` to clear.
        label: Option<String>,
    },
    /// Sets mute and bypass.
    SetFlags {
        /// The node.
        node: NodeRef,
        /// New mute state, if changing.
        #[serde(default)]
        muted: Option<bool>,
        /// New bypass state, if changing.
        #[serde(default)]
        bypass: Option<bool>,
    },
    /// Moves a node to another version of its operator.
    SetOpVersion {
        /// The node.
        node: NodeRef,
        /// The version to use.
        version: u32,
    },
    /// Declares or removes a named output.
    SetOutput {
        /// The output name.
        name: String,
        /// The source, or `None` to remove the output.
        from: Option<OutRef>,
    },
    /// Adds a material definition.
    DefineMaterial {
        /// Named values.
        def: BTreeMap<String, Value>,
    },
    /// Removes a material definition.
    RemoveMaterial {
        /// The material.
        id: MaterialId,
    },
    /// Moves a node in the editor.
    EditorSetPosition {
        /// The node.
        node: NodeRef,
        /// Horizontal position.
        x: f64,
        /// Vertical position.
        y: f64,
    },
    /// Collapses or expands a node in the editor.
    EditorSetCollapsed {
        /// The node.
        node: NodeRef,
        /// Whether it is drawn collapsed.
        collapsed: bool,
    },
}

/// A set of commands applied atomically against one revision.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommandBatch {
    /// The document this batch is for.
    pub doc: DocId,
    /// The revision the author saw.
    pub base_revision: u64,
    /// Idempotency key: a repeated key applies nothing a second time.
    pub transaction: TransactionId,
    /// What the batch does, for the history list.
    #[serde(default)]
    pub label: Option<String>,
    /// The edits, in order.
    pub commands: Vec<Command>,
}

impl CommandBatch {
    /// Reads a batch from JSON, enforcing the size limit.
    ///
    /// # Errors
    /// A `parse.syntax` or `budget.source` diagnostic.
    pub fn from_json(text: &str) -> Result<Self, Box<Diagnostic>> {
        if text.len() > MAX_BATCH_BYTES {
            #[expect(clippy::cast_precision_loss, reason = "sizes are far below 2^53")]
            return Err(Box::new(
                Diagnostic::error("budget.source", "the batch is too large")
                    .with_limit(MAX_BATCH_BYTES as f64, text.len() as f64),
            ));
        }
        serde_json::from_str(text).map_err(|e| {
            Box::new(Diagnostic::error(
                "parse.syntax",
                format!("invalid batch: {e}"),
            ))
        })
    }
}

/// What happened to a batch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Applied; the document is now at `revision`.
    Committed {
        /// The new revision.
        revision: u64,
        /// The batch's key.
        transaction: TransactionId,
        /// Nodes added, removed or changed.
        changed: Vec<NodeId>,
    },
    /// Refused; the document is unchanged.
    Rejected {
        /// Why.
        diagnostics: Vec<Diagnostic>,
    },
    /// The author's revision is not current; nothing was applied.
    Conflict {
        /// The revision the batch was based on.
        base_revision: u64,
        /// The current revision.
        current_revision: u64,
        /// Nodes changed since then, when the history still covers it.
        changed_since: Vec<NodeId>,
        /// Whether `changed_since` is complete (false when older history was dropped).
        complete: bool,
    },
    /// The key was already applied; nothing changed this time.
    AlreadyApplied {
        /// The revision the first application produced.
        revision: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_use_the_documented_wire_form() {
        let node = NodeId::parse("n_0123abcd").unwrap();
        let json =
            serde_json::json!({ "set_param": { "node": node, "param": "radius", "value": 2.0 } });
        let command: Command = serde_json::from_value(json).unwrap();
        assert_eq!(
            command,
            Command::SetParam {
                node: node.into(),
                param: "radius".into(),
                value: Value::Num(2.0)
            }
        );
    }

    #[test]
    fn aliases_and_ids_are_told_apart() {
        assert!(matches!(
            NodeRef::try_from("$outer".to_owned()),
            Ok(NodeRef::Alias(_))
        ));
        assert!(matches!(
            NodeRef::try_from("n_0123abcd".to_owned()),
            Ok(NodeRef::Id(_))
        ));
        for bad in [
            "$",
            "$has space",
            "outer",
            "$".to_owned().repeat(2).as_str(),
        ] {
            assert!(NodeRef::try_from(bad.to_owned()).is_err(), "{bad}");
        }
        assert!(Alias::parse(&format!("${}", "a".repeat(33))).is_err());
    }

    #[test]
    fn unknown_fields_and_commands_are_refused() {
        assert!(
            serde_json::from_str::<Command>(r#"{"add_node":{"op":"solid.box@1","evil":1}}"#)
                .is_err()
        );
        assert!(serde_json::from_str::<Command>(r#"{"run_code":{}}"#).is_err());
    }

    #[test]
    fn oversized_and_malformed_batches_are_diagnosed() {
        let big = " ".repeat(MAX_BATCH_BYTES + 1);
        assert_eq!(
            CommandBatch::from_json(&big).unwrap_err().code,
            "budget.source"
        );
        assert_eq!(
            CommandBatch::from_json("{").unwrap_err().code,
            "parse.syntax"
        );
    }
}
