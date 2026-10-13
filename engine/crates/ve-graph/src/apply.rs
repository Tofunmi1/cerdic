//! Applying a batch of commands to a candidate copy of the document.
//!
//! The committed document is never touched here: callers receive a finished candidate or the
//! diagnostics that stopped it. Commands run in order and the first failure stops the batch.

use std::collections::{BTreeMap, BTreeSet};

use crate::command::{Command, GroupRef, InputRef, NodeRef, OutRef};
use crate::diag::Diagnostic;
use crate::doc::{Document, PortRef};
use crate::group_check;
use crate::groups;
use crate::id::{GroupId, NodeId};
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
    group_aliases: BTreeMap<String, GroupId>,
}

impl<'a> Ctx<'a> {
    /// A working copy of `doc` for commands to change.
    pub(crate) fn new(doc: &Document, reg: &'a OperatorRegistry) -> Self {
        Self {
            doc: doc.clone(),
            reg,
            changed: BTreeSet::new(),
            aliases: BTreeMap::new(),
            group_aliases: BTreeMap::new(),
        }
    }

    fn group(&self, reference: &GroupRef) -> Result<GroupId, Fail> {
        match reference {
            GroupRef::Id(id) => Ok(id.clone()),
            GroupRef::Alias(alias) => {
                self.group_aliases
                    .get(alias.as_str())
                    .cloned()
                    .ok_or_else(|| {
                        fail(
                            "group.unknown",
                            format!("{} is not defined earlier in this batch", alias.as_str()),
                        )
                    })
            }
        }
    }

    fn alias_free(&self, alias: Option<&crate::command::Alias>) -> Result<(), Fail> {
        match alias {
            Some(alias)
                if self.aliases.contains_key(alias.as_str())
                    || self.group_aliases.contains_key(alias.as_str()) =>
            {
                Err(fail(
                    "graph.duplicate_id",
                    format!("{} is already used in this batch", alias.as_str()),
                ))
            }
            _ => Ok(()),
        }
    }

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
    group: Option<&GroupRef>,
) -> Result<(), Fail> {
    ctx.alias_free(alias)?;
    let owner = group.map(|g| ctx.group(g)).transpose()?;
    let id = nodes::add_node(ctx, op, params, label)?;
    if let Some(owner) = owner {
        groups::join(ctx, &id, &owner)?;
    }
    if let Some(alias) = alias {
        ctx.aliases.insert(alias.as_str().to_owned(), id);
    }
    Ok(())
}

fn add_group_node(
    ctx: &mut Ctx<'_>,
    reference: &GroupRef,
    label: Option<&str>,
    alias: Option<&crate::command::Alias>,
    into: Option<&GroupRef>,
) -> Result<(), Fail> {
    ctx.alias_free(alias)?;
    let (group, into) = (
        ctx.group(reference)?,
        into.map(|g| ctx.group(g)).transpose()?,
    );
    let id = groups::add_instance(ctx, &group, label, into.as_ref())?;
    if let Some(alias) = alias {
        ctx.aliases.insert(alias.as_str().to_owned(), id);
    }
    Ok(())
}

fn create_group(
    ctx: &mut Ctx<'_>,
    name: &str,
    members: &[NodeRef],
    output: &NodeRef,
    alias: Option<&crate::command::Alias>,
) -> Result<(), Fail> {
    ctx.alias_free(alias)?;
    let members: Vec<NodeId> = members
        .iter()
        .map(|m| ctx.node(m))
        .collect::<Result<_, _>>()?;
    let id = groups::create_group(ctx, name, &members, &ctx.node(output)?)?;
    if let Some(alias) = alias {
        ctx.group_aliases.insert(alias.as_str().to_owned(), id);
    }
    Ok(())
}

fn run_group(ctx: &mut Ctx<'_>, command: &Command) -> Result<(), Fail> {
    match command {
        Command::CreateGroup {
            name,
            members,
            output,
            alias,
        } => create_group(ctx, name, members, output, alias.as_ref()),
        Command::RemoveGroup { group } => {
            let group = ctx.group(group)?;
            groups::remove_group(ctx, &group)
        }
        Command::SetGroupName { group, name } => {
            let group = ctx.group(group)?;
            groups::set_group_name(ctx, &group, name)
        }
        Command::ExposeParam {
            group,
            name,
            node,
            param,
        } => {
            let (group, node) = (ctx.group(group)?, ctx.node(node)?);
            groups::expose_param(ctx, &group, name, &node, param)
        }
        Command::UnexposeParam { group, name } => {
            let group = ctx.group(group)?;
            groups::unexpose_param(ctx, &group, name)
        }
        Command::SetGroupOutput { group, node } => {
            let (group, node) = (ctx.group(group)?, ctx.node(node)?);
            groups::set_group_output(ctx, &group, &node)
        }
        Command::AddInstance {
            group,
            label,
            alias,
            into,
        } => add_group_node(ctx, group, label.as_deref(), alias.as_ref(), into.as_ref()),
        Command::SetInstanceParam { node, param, value } => {
            let node = ctx.node(node)?;
            groups::set_instance_param(ctx, &node, param, value)
        }
        Command::ClearInstanceParam { node, param } => {
            let node = ctx.node(node)?;
            groups::clear_instance_param(ctx, &node, param)
        }
        Command::UpgradeGroup { node } => {
            let node = ctx.node(node)?;
            groups::upgrade_group(ctx, &node)
        }
        _ => Ok(()),
    }
}

fn run(ctx: &mut Ctx<'_>, command: &Command) -> Result<(), Fail> {
    match command {
        Command::AddNode {
            op,
            params,
            label,
            alias,
            group,
        } => add(
            ctx,
            op,
            params,
            label.as_deref(),
            alias.as_ref(),
            group.as_ref(),
        ),
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
        group_command => run_group(ctx, group_command),
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
    let mut ctx = Ctx::new(doc, reg);
    for (index, command) in commands.iter().enumerate() {
        run(&mut ctx, command)
            .and_then(|()| group_check::check(&ctx))
            .map_err(|d| vec![d.at_command(index)])?;
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
