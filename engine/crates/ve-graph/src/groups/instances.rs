//! Instance commands: adding an instance, overriding and clearing its parameters, and upgrading it
//! to its group's current version.

use std::collections::{BTreeMap, BTreeSet};

use super::{group_of, name_ok};
use crate::apply::{Ctx, Fail, fail};
use crate::diag::Diagnostic;
use crate::doc::{Document, Flags, Node};
use crate::group::{GroupParam, INSTANCE_OP, Instance};
use crate::id::{GroupId, NodeId};
use crate::limits::{MAX_NODES, MAX_STRING_BYTES, count};
use crate::nodes::unknown_node;
use crate::spec::{OpId, OperatorRegistry};
use crate::value::Value;

/// Adds an instance of `id` at its current version; `into` makes it a member of another group.
pub(crate) fn add_instance(
    ctx: &mut Ctx<'_>,
    id: &GroupId,
    label: Option<&str>,
    into: Option<&GroupId>,
) -> Result<NodeId, Fail> {
    let version = group_of(ctx, id)?.version;
    if let Some(into) = into {
        group_of(ctx, into)?;
    }
    if let Some(label) = label {
        name_ok(label, "the label", MAX_STRING_BYTES)?;
    }
    if ctx.doc.nodes.len() >= MAX_NODES {
        return Err(Box::new(
            Diagnostic::error("budget.nodes", "the document has too many nodes")
                .with_limit(count(MAX_NODES), count(ctx.doc.nodes.len() + 1)),
        ));
    }
    let op = OpId::new(INSTANCE_OP, 1).map_err(|e| fail("parse.unknown_op", e.to_string()))?;
    let node_id = ctx
        .doc
        .issue_node_id()
        .ok_or_else(|| fail("budget.nodes", "node ids are used up"))?;
    ctx.doc.nodes.push(Node {
        id: node_id.clone(),
        op,
        label: label.map(str::to_owned),
        params: BTreeMap::new(),
        inputs: BTreeMap::new(),
        flags: Flags::default(),
        group: into.cloned(),
        instance: Some(Instance {
            group: id.clone(),
            version,
            overrides: BTreeMap::new(),
        }),
    });
    ctx.changed.insert(node_id.clone());
    Ok(node_id)
}

/// The instance behind `node`, its group, and the exposed parameter `param` of it.
fn exposed<'c>(
    ctx: &'c Ctx<'_>,
    node: &NodeId,
    param: &str,
) -> Result<(&'c Instance, &'c GroupParam), Fail> {
    let found = ctx.doc.node(node).ok_or_else(|| unknown_node(node))?;
    let Some(instance) = &found.instance else {
        return Err(not_instance(node));
    };
    let group = group_of(ctx, &instance.group)?;
    if instance.version != group.version {
        return Err(Box::new(
            Diagnostic::error(
                "group.outdated",
                format!(
                    "{node} was made against version {} of its group, which is now version {}",
                    instance.version, group.version
                ),
            )
            .at_node(node)
            .suggest("upgrade the instance first"),
        ));
    }
    let Some(found) = group.params.iter().find(|p| p.name == param) else {
        let valid: Vec<&str> = group.params.iter().map(|p| p.name.as_str()).collect();
        return Err(Box::new(
            Diagnostic::error("group.unknown_param", format!("{param:?} is not exposed"))
                .at_node(node)
                .suggest(format!("exposed: {}", valid.join(", "))),
        ));
    };
    Ok((instance, found))
}

fn not_instance(node: &NodeId) -> Fail {
    Box::new(
        Diagnostic::error("group.not_instance", format!("{node} is not an instance")).at_node(node),
    )
}

pub(crate) fn set_instance_param(
    ctx: &mut Ctx<'_>,
    node: &NodeId,
    param: &str,
    value: &Value,
) -> Result<(), Fail> {
    let (_, exposed) = exposed(ctx, node, param)?;
    let checked = check_override(ctx, exposed, value, node)?;
    if let Some(instance) = ctx.doc.node_mut(node).and_then(|n| n.instance.as_mut()) {
        instance.overrides.insert(param.to_owned(), checked);
    }
    ctx.changed.insert(node.clone());
    Ok(())
}

/// The value as the exposed member parameter would accept it, or the reason it would not.
pub(crate) fn check_override(
    ctx: &Ctx<'_>,
    exposed: &GroupParam,
    value: &Value,
    node: &NodeId,
) -> Result<Value, Fail> {
    let holder = ctx
        .doc
        .node(&exposed.node)
        .ok_or_else(|| unknown_node(&exposed.node))?;
    let ty = ctx
        .reg
        .get(&holder.op)
        .and_then(|spec| spec.param(&exposed.param))
        .map(|p| p.ty)
        .ok_or_else(|| fail("parse.unknown_param", exposed.param.clone()))?;
    ty.check(value)
        .map_err(|issue| Box::new(Diagnostic::from_issue(issue, Some(node), &exposed.name)))
}

pub(crate) fn clear_instance_param(
    ctx: &mut Ctx<'_>,
    node: &NodeId,
    param: &str,
) -> Result<(), Fail> {
    exposed(ctx, node, param)?;
    let removed = ctx
        .doc
        .node_mut(node)
        .and_then(|n| n.instance.as_mut())
        .and_then(|i| i.overrides.remove(param));
    if removed.is_none() {
        return Err(Box::new(
            Diagnostic::error(
                "group.unknown_param",
                format!("{param:?} has no override on this instance"),
            )
            .at_node(node),
        ));
    }
    ctx.changed.insert(node.clone());
    Ok(())
}

/// What upgrading the instance `node` would drop, so an editor can show the conflict before applying;
/// `None` when `node` is not an instance. Empty when nothing would be lost.
#[must_use]
pub fn upgrade_preview(
    doc: &Document,
    reg: &OperatorRegistry,
    node: &NodeId,
) -> Option<UpgradePreview> {
    doc.node(node)?.instance.as_ref()?;
    preview(&Ctx::new(doc, reg), node).ok()
}

/// What `upgrade_group` would do to an instance: the overrides it would drop and the input links it
/// would remove, so an editor can show the conflict before applying.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UpgradePreview {
    /// Overrides that no longer match an exposed parameter, or no longer fit it.
    pub dropped_overrides: Vec<String>,
    /// Input ports that no longer exist.
    pub dropped_links: Vec<String>,
}

pub(crate) fn upgrade_group(ctx: &mut Ctx<'_>, node: &NodeId) -> Result<(), Fail> {
    let found = ctx.doc.node(node).ok_or_else(|| unknown_node(node))?;
    let Some(instance) = found.instance.clone() else {
        return Err(not_instance(node));
    };
    let group = group_of(ctx, &instance.group)?;
    if instance.version == group.version {
        return Err(Box::new(
            Diagnostic::error("group.up_to_date", format!("{node} is already current"))
                .at_node(node),
        ));
    }
    let version = group.version;
    let preview = preview(ctx, node)?;
    if let Some(found) = ctx.doc.node_mut(node) {
        if let Some(instance) = found.instance.as_mut() {
            instance.version = version;
            instance
                .overrides
                .retain(|name, _| !preview.dropped_overrides.contains(name));
        }
        found
            .inputs
            .retain(|port, _| !preview.dropped_links.contains(port));
    }
    ctx.changed.insert(node.clone());
    Ok(())
}

/// What upgrading `node` would drop; empty when it is current.
fn preview(ctx: &Ctx<'_>, node: &NodeId) -> Result<UpgradePreview, Fail> {
    let found = ctx.doc.node(node).ok_or_else(|| unknown_node(node))?;
    let Some(instance) = &found.instance else {
        return Ok(UpgradePreview::default());
    };
    let group = group_of(ctx, &instance.group)?;
    let mut out = UpgradePreview::default();
    for (name, value) in &instance.overrides {
        let fits = group
            .params
            .iter()
            .find(|p| &p.name == name)
            .is_some_and(|p| check_override(ctx, p, value, node).is_ok());
        if !fits {
            out.dropped_overrides.push(name.clone());
        }
    }
    let inputs: BTreeSet<String> = ctx
        .doc
        .group_inputs(&group.id)
        .into_iter()
        .map(|i| i.name)
        .collect();
    out.dropped_links = found
        .inputs
        .keys()
        .filter(|port| !inputs.contains(*port))
        .cloned()
        .collect();
    Ok(out)
}
