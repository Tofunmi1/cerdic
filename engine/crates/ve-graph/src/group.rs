//! Group definitions and instances (see ADR 0013).
//!
//! A group's members are ordinary nodes of the document that carry its ID; the group itself holds only
//! the interface: which member is the output and which member parameters an instance may override.
//! An instance is an ordinary `group.use@1` node pinned to a version of its group.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::doc::{Document, Node};
use crate::id::{GroupId, NodeId};
use crate::value::Value;

/// The operator of an instance node.
pub const INSTANCE_OP: &str = "group.use";

/// The operator of a group's input placeholder; its `name` parameter is the port name.
pub const INPUT_OP: &str = "group.input";

/// A parameter of a member that instances may override, under a name of the group's choosing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct GroupParam {
    /// The name instances use.
    pub name: String,
    /// The member holding the parameter.
    pub node: NodeId,
    /// The member's parameter.
    pub param: String,
}

/// A reusable assembly. Its members are the document's nodes with `group` set to this ID.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct Group {
    /// Stable identity.
    pub id: GroupId,
    /// A display name; never part of identity or evaluation.
    pub name: String,
    /// Increases whenever the interface changes; instances pin the version they were made against.
    pub version: u32,
    /// The member whose result is the group's result.
    pub output: NodeId,
    /// What instances may override.
    #[serde(default)]
    pub params: Vec<GroupParam>,
}

/// What makes a `group.use@1` node an instance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct Instance {
    /// The group.
    pub group: GroupId,
    /// The version of the interface this instance was made against.
    pub version: u32,
    /// Overridden parameter values by exposed name.
    #[serde(default)]
    pub overrides: BTreeMap<String, Value>,
}

/// The input an instance may bind: a member of operator `group.input@1`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupInput {
    /// The port name an instance links to.
    pub name: String,
    /// The placeholder member.
    pub node: NodeId,
}

impl Document {
    /// The group with this ID.
    #[must_use]
    pub fn group(&self, id: &GroupId) -> Option<&Group> {
        self.groups.iter().find(|g| &g.id == id)
    }

    /// The members of a group, in creation order.
    pub fn members<'a>(&'a self, id: &'a GroupId) -> impl Iterator<Item = &'a Node> {
        self.nodes
            .iter()
            .filter(move |n| n.group.as_ref() == Some(id))
    }

    /// The instance nodes of a group.
    pub fn instances_of<'a>(&'a self, id: &'a GroupId) -> impl Iterator<Item = &'a Node> {
        self.nodes
            .iter()
            .filter(move |n| n.instance.as_ref().is_some_and(|i| &i.group == id))
    }

    /// The inputs of a group: its `group.input@1` members by their names.
    #[must_use]
    pub fn group_inputs(&self, id: &GroupId) -> Vec<GroupInput> {
        self.members(id)
            .filter(|n| n.op.family().as_str() == INPUT_OP)
            .filter_map(|n| match n.params.get("name") {
                Some(Value::Str(name)) => Some(GroupInput {
                    name: name.clone(),
                    node: n.id.clone(),
                }),
                _ => None,
            })
            .collect()
    }

    /// Issues a group ID, or `None` if the 40-bit space is used up.
    pub(crate) fn issue_group_id(&mut self) -> Option<GroupId> {
        while self.ids.next < (1 << 40) {
            let id = GroupId::generated(self.ids.next, self.ids.salt);
            self.ids.next += 1;
            if self.group(&id).is_none() {
                return Some(id);
            }
        }
        None
    }

    pub(crate) fn group_mut(&mut self, id: &GroupId) -> Option<&mut Group> {
        self.groups.iter_mut().find(|g| &g.id == id)
    }
}
