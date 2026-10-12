//! Applying a batch of commands to a candidate copy of the document.
//!
//! The committed document is never touched here: callers receive a finished candidate or the
//! diagnostics that stopped it. Commands run in order and the first failure stops the batch.

use std::collections::{BTreeMap, BTreeSet};

use crate::command::{Command, InputRef, NodeRef, OutRef};
use crate::diag::Diagnostic;
use crate::doc::{Document, PortRef};
use crate::id::NodeId;
use crate::limits::MAX_DOCUMENT_BYTES;
use crate::links;
use crate::nodes;
use crate::spec::OperatorRegistry;

/// A failed command: boxed because diagnostics are large and failures are rare.
pub(crate) type Fail = Box<Diagnostic>;

/// An input port with its node resolved to a real ID.
pub(crate) struct Target {
    pub(crate) node: NodeId,
    pub(crate) port: String,
    pub(crate) index: Option<usize>,
}

/// The state commands work on.
pub(crate) struct Ctx<'a> {
    pub(crate) doc: Document,
    pub(crate) reg: &'a OperatorRegistry,
    pub(crate) changed: BTreeSet<NodeId>,
    aliases: BTreeMap<String, NodeId>,
}

impl Ctx<'_> {
    fn node(&self, reference: &NodeRef) -> Result<NodeId, Fail> {
        match reference {
            NodeRef::Id(id) => Ok(id.clone()),
            NodeRef::Alias(alias) => self.aliases.get(alias.as_str()).cloned().ok_or_else(|| {
                fail(
                    "graph.unknown_node",
                    format!("{} is not defined earlier in this batch", alias.as_str()),
                )
            }),
        }
    }

    fn out(&self, reference: &OutRef) -> Result<PortRef, Fail> {
        Ok(PortRef {
            node: self.node(&reference.node)?,
            port: reference.port.clone(),
        })
    }

    fn target(&self, reference: &InputRef) -> Result<Target, Fail> {
        Ok(Target {
            node: self.node(&reference.node)?,
            port: reference.port.clone(),
            index: reference.index,
        })
    }
}

/// A candidate document and what changed to produce it.
pub(crate) struct Applied {
    pub(crate) doc: Document,
    pub(crate) changed: Vec<NodeId>,
}

pub(crate) fn fail(code: &str, message: impl Into<String>) -> Fail {
    Box::new(Diagnostic::error(code, message))
}

fn add(
    ctx: &mut Ctx<'_>,
    op: &str,
    params: &BTreeMap<String, crate::value::Value>,
    label: Option<&str>,
    alias: Option<&crate::command::Alias>,
) -> Result<(), Fail> {
    if let Some(alias) = alias
        && ctx.aliases.contains_key(alias.as_str())
    {
        return Err(fail(
            "graph.duplicate_id",
            format!("{} is already used in this batch", alias.as_str()),
        ));
    }
    let id = nodes::add_node(ctx, op, params, label)?;
    if let Some(alias) = alias {
        ctx.aliases.insert(alias.as_str().to_owned(), id);
    }
    Ok(())
}

fn run(ctx: &mut Ctx<'_>, command: &Command) -> Result<(), Fail> {
    match command {
        Command::AddNode {
            op,
            params,
            label,
            alias,
        } => add(ctx, op, params, label.as_deref(), alias.as_ref()),
        Command::RemoveNode { node, cascade } => {
            let node = ctx.node(node)?;
            nodes::remove_node(ctx, &node, *cascade)
        }
        Command::Connect { from, to } => {
            let (from, to) = (ctx.out(from)?, ctx.target(to)?);
            links::connect(ctx, &from, &to)
        }
        Command::Disconnect { to } => {
            let to = ctx.target(to)?;
            links::disconnect(ctx, &to)
        }
        Command::SetParam { node, param, value } => {
            let node = ctx.node(node)?;
            nodes::set_param(ctx, &node, param, value)
        }
        Command::SetLabel { node, label } => {
            let node = ctx.node(node)?;
            nodes::set_label(ctx, &node, label.as_deref())
        }
        Command::SetFlags {
            node,
            muted,
            bypass,
        } => {
            let node = ctx.node(node)?;
            nodes::set_flags(ctx, &node, *muted, *bypass)
        }
        Command::SetOpVersion { node, version } => {
            let node = ctx.node(node)?;
            nodes::set_op_version(ctx, &node, *version)
        }
        Command::SetOutput { name, from } => {
            let from = from.as_ref().map(|f| ctx.out(f)).transpose()?;
            nodes::set_output(ctx, name, from.as_ref())
        }
        Command::DefineMaterial { def } => nodes::define_material(ctx, def),
        Command::RemoveMaterial { id } => nodes::remove_material(ctx, id),
        Command::EditorSetPosition { node, x, y } => {
            let node = ctx.node(node)?;
            nodes::editor_set_position(ctx, &node, *x, *y)
        }
        Command::EditorSetCollapsed { node, collapsed } => {
            let node = ctx.node(node)?;
            nodes::editor_set_collapsed(ctx, &node, *collapsed)
        }
    }
}

/// Applies `commands` to a copy of `doc`.
///
/// # Errors
/// The diagnostics of the first command that failed (with its index), or of the size check.
pub(crate) fn apply_batch(
    doc: &Document,
    reg: &OperatorRegistry,
    commands: &[Command],
) -> Result<Applied, Vec<Diagnostic>> {
    let mut ctx = Ctx {
        doc: doc.clone(),
        reg,
        changed: BTreeSet::new(),
        aliases: BTreeMap::new(),
    };
    for (index, command) in commands.iter().enumerate() {
        run(&mut ctx, command).map_err(|d| vec![d.at_command(index)])?;
    }
    let size = ctx.doc.canonical_json().len();
    if size > MAX_DOCUMENT_BYTES {
        #[expect(clippy::cast_precision_loss, reason = "sizes are far below 2^53")]
        return Err(vec![
            Diagnostic::error("budget.source", "the document would be too large")
                .with_limit(MAX_DOCUMENT_BYTES as f64, size as f64),
        ]);
    }
    Ok(Applied {
        doc: ctx.doc,
        changed: ctx.changed.into_iter().collect(),
    })
}
