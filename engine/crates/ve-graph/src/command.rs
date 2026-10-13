//! The typed command set (see `docs/modeling/commands.md`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::diag::Diagnostic;
use crate::id::{DocId, MaterialId, NodeId, TransactionId};
use crate::limits::MAX_BATCH_BYTES;
use crate::value::Value;

mod refs;

pub use refs::{Alias, GroupRef, InputRef, NodeRef, OutRef};

/// One edit. Batches apply all of their commands or none.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
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
        /// Makes the node a member of this group's definition instead of a top-level node.
        #[serde(default)]
        group: Option<GroupRef>,
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
    /// Turns existing nodes into the definition of a new group (version 1). The members must read
    /// only each other, and nothing outside may read them.
    CreateGroup {
        /// Display name.
        name: String,
        /// The nodes that become members.
        members: Vec<NodeRef>,
        /// The member whose result is the group's result (a solid).
        output: NodeRef,
        /// A name later commands in this batch can use for the new group.
        #[serde(default)]
        alias: Option<Alias>,
    },
    /// Removes a group and its members; refused while instances exist.
    RemoveGroup {
        /// The group.
        group: GroupRef,
    },
    /// Renames a group.
    SetGroupName {
        /// The group.
        group: GroupRef,
        /// The new name.
        name: String,
    },
    /// Lets instances override a member's parameter under `name`. Changes the interface.
    ExposeParam {
        /// The group.
        group: GroupRef,
        /// The name instances use.
        name: String,
        /// The member holding the parameter.
        node: NodeRef,
        /// The member's parameter.
        param: String,
    },
    /// Stops exposing a parameter. Changes the interface.
    UnexposeParam {
        /// The group.
        group: GroupRef,
        /// The exposed name.
        name: String,
    },
    /// Picks another member as the group's result. Changes the interface.
    SetGroupOutput {
        /// The group.
        group: GroupRef,
        /// The member.
        node: NodeRef,
    },
    /// Adds an instance of a group at its current version.
    AddInstance {
        /// The group.
        group: GroupRef,
        /// Display name.
        #[serde(default)]
        label: Option<String>,
        /// A name later commands in this batch can use for the new node.
        #[serde(default)]
        alias: Option<Alias>,
        /// Makes the instance a member of this group's definition (a nested group) instead of a
        /// top-level node.
        #[serde(default)]
        into: Option<GroupRef>,
    },
    /// Overrides an exposed parameter on one instance.
    SetInstanceParam {
        /// The instance.
        node: NodeRef,
        /// The exposed name.
        param: String,
        /// The value, checked against the member parameter's type and range.
        value: Value,
    },
    /// Removes an override, so the instance follows the definition again.
    ClearInstanceParam {
        /// The instance.
        node: NodeRef,
        /// The exposed name.
        param: String,
    },
    /// Moves an instance to its group's current version, keeping the overrides that still match an
    /// exposed parameter and dropping the rest.
    UpgradeGroup {
        /// The instance.
        node: NodeRef,
    },
}

/// A set of commands applied atomically against one revision.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
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
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
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
        assert!(matches!(
            GroupRef::try_from("g_0123abcd".to_owned()),
            Ok(GroupRef::Id(_))
        ));
        assert!(GroupRef::try_from("n_0123abcd".to_owned()).is_err());
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
