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
mod id;
mod limits;
mod links;
mod nodes;
mod session;
mod spec;
mod value;

pub use builtin::builtin_registry;
pub use command::{Alias, Command, CommandBatch, InputRef, NodeRef, OutRef, Outcome};
pub use diag::{Diagnostic, Severity};
pub use doc::{
    Document, EditorLayer, EvalHash, Flags, IdState, Material, Node, OUT_PORT, Output, PortRef,
    SCHEMA,
};
pub use id::{DocId, IdError, MaterialId, NodeId, TransactionId};
pub use limits::{
    MAX_BATCH_BYTES, MAX_COMMANDS, MAX_DOCUMENT_BYTES, MAX_HISTORY, MAX_HISTORY_BYTES, MAX_NODES,
};
pub use session::{Moved, Session};
pub use spec::{
    Arity, OpId, OpIdError, OperatorRegistry, OperatorSpec, OutputSpec, ParamSpec, PortSpec,
    PortType, Readiness,
};
pub use value::{ParamIssue, ParamType, Unit, Value};
