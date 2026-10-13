//! Names a command can use for the things it works on: aliases for what a batch adds, and references
//! that accept either a real ID or an alias.

use serde::{Deserialize, Serialize};

use crate::id::{GroupId, IdError, NodeId};

/// A name valid only inside one batch for a node or group the batch adds: `$` plus 1 to 32
/// characters from `[A-Za-z0-9_]`. It lets later commands in the same batch refer to something whose
/// ID is issued when the batch is applied.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS), ts(type = "string"))]
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
#[cfg_attr(feature = "typescript", derive(ts_rs::TS), ts(type = "string"))]
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

/// A group named in a command: by its real ID, or by an alias the same batch defined.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS), ts(type = "string"))]
pub enum GroupRef {
    /// An existing group.
    Id(GroupId),
    /// A group created earlier in this batch.
    Alias(Alias),
}

impl TryFrom<String> for GroupRef {
    type Error = IdError;
    fn try_from(s: String) -> Result<Self, IdError> {
        if s.starts_with('$') {
            Alias::parse(&s).map(Self::Alias)
        } else {
            GroupId::parse(&s).map(Self::Id)
        }
    }
}

impl From<GroupRef> for String {
    fn from(r: GroupRef) -> String {
        match r {
            GroupRef::Id(id) => id.into(),
            GroupRef::Alias(alias) => alias.into(),
        }
    }
}

impl From<GroupId> for GroupRef {
    fn from(id: GroupId) -> Self {
        Self::Id(id)
    }
}

/// An output port named in a command.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
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
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
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
