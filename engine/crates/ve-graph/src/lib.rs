//! The authored document and the commands that change it.
//!
//! One versioned document with stable IDs; every edit is a [`CommandBatch`] checked against a
//! revision, applied atomically to a candidate copy, and recorded for undo. Engine-agnostic: no Bevy,
//! no platform code. See `docs/modeling/` for the contracts this crate implements.

mod apply;
mod builtin;
mod command;
mod diag;
mod doc;
mod expand;
mod group;
mod group_check;
mod groups;
mod id;
mod limits;
mod links;
mod nodes;
mod session;
mod spec;
mod store;
mod text;
mod typed_api;
mod value;

pub use builtin::builtin_registry;
pub use command::{Alias, Command, CommandBatch, GroupRef, InputRef, NodeRef, OutRef, Outcome};
pub use diag::{Diagnostic, Severity, Span};
pub use doc::{
    Document, EditorLayer, EvalHash, Flags, IdState, Material, Node, OUT_PORT, Output, PortRef,
    SCHEMA,
};
pub use expand::{Expanded, Origin, expand};
pub use group::{Group, GroupInput, GroupParam, INPUT_OP, INSTANCE_OP, Instance};
pub use groups::{UpgradePreview, upgrade_preview};
pub use id::{DocId, GroupId, IdError, MaterialId, NodeId, TransactionId};
pub use limits::{
    MAX_BATCH_BYTES, MAX_COMMANDS, MAX_DOCUMENT_BYTES, MAX_EXPANDED_NODES, MAX_GROUP_DEPTH,
    MAX_HISTORY, MAX_HISTORY_BYTES, MAX_NODES,
};
pub use session::{History, HistoryItem, Moved, Session};
pub use spec::{
    Arity, OpId, OpIdError, OperatorRegistry, OperatorSpec, OutputSpec, ParamSpec, PortSpec,
    PortType, Readiness, Socket,
};
pub use store::{Backend, BackendError, DocStore, Loaded, MemoryBackend, MigrationStep, Migrator};
pub use text::{
    TextBatch, parse_text, parse_text_in, to_text, to_text_compact, to_text_node_lines,
};
pub use typed_api::operators_d_ts;
pub use value::{ParamIssue, ParamType, Unit, Value};
