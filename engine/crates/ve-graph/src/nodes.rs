//! Commands that add, remove and edit nodes.

mod checks;

pub(crate) use checks::{
    define_material, editor_set_collapsed, editor_set_position, remove_material, set_output,
};

use std::collections::{BTreeMap, BTreeSet};

use crate::apply::{Ctx, Fail, fail};
use crate::diag::Diagnostic;
use crate::doc::{Flags, Node};
use crate::group::INSTANCE_OP;
use crate::groups;
use crate::id::NodeId;
use crate::limits::{MAX_NODES, MAX_STRING_BYTES, count};
use crate::links;
use crate::spec::{OpId, OperatorSpec};
use crate::value::Value;

pub(crate) fn unknown_node(id: &NodeId) -> Fail {
    Box::new(Diagnostic::error("graph.unknown_node", format!("there is no node {id}")).at_node(id))
}

fn unknown_op(ctx: &Ctx<'_>, text: &str) -> Fail {
    let hints = ctx.reg.suggest(text);
    let mut d = Diagnostic::error("parse.unknown_op", format!("unknown operator {text:?}"));
    if !hints.is_empty() {
        d = d.suggest(format!("did you mean {}?", hints.join(", ")));
    }
    Box::new(d)
}

fn spec_of<'r>(ctx: &Ctx<'r>, op: &OpId) -> Result<&'r OperatorSpec, Fail> {
    ctx.reg
        .get(op)
        .ok_or_else(|| unknown_op(ctx, &op.to_string()))
}

fn existing<'c>(ctx: &'c Ctx<'_>, id: &NodeId) -> Result<&'c Node, Fail> {
    ctx.doc.node(id).ok_or_else(|| unknown_node(id))
}

fn text_ok(text: &str, what: &str) -> Result<(), Fail> {
    if text.len() > MAX_STRING_BYTES {
        return Err(Box::new(
            Diagnostic::error("param.range", format!("{what} is too long"))
                .with_limit(count(MAX_STRING_BYTES), count(text.len()))
                .suggest(format!(
                    "shorten {what} to at most {MAX_STRING_BYTES} bytes"
                )),
        ));
    }
    Ok(())
}

/// Builds a node's parameter map: checks every given value, fills defaults, requires the rest.
fn build_params(
    spec: &OperatorSpec,
    given: &BTreeMap<String, Value>,
    node: Option<&NodeId>,
) -> Result<BTreeMap<String, Value>, Fail> {
    for key in given.keys() {
        if spec.param(key).is_none() {
            let valid: Vec<&str> = spec.params.iter().map(|p| p.name).collect();
            return Err(Box::new(
                Diagnostic::error(
                    "parse.unknown_param",
                    format!("{} has no parameter {key:?}", spec.id),
                )
                .suggest(format!("parameters: {}", valid.join(", "))),
            ));
        }
    }
    let mut out = BTreeMap::new();
    for param in &spec.params {
        let value = match given.get(param.name).or(param.default.as_ref()) {
            Some(v) => param
                .ty
                .check(v)
                .map_err(|issue| Box::new(Diagnostic::from_issue(issue, node, param.name)))?,
            None => {
                return Err(Box::new(
                    Diagnostic::error(
                        "parse.missing_param",
                        format!("{} needs the parameter {:?}", spec.id, param.name),
                    )
                    .suggest(param.doc),
                ));
            }
        };
        out.insert(param.name.to_owned(), value);
    }
    Ok(out)
}

pub(crate) fn add_node(
    ctx: &mut Ctx<'_>,
    op: &str,
    params: &BTreeMap<String, Value>,
    label: Option<&str>,
) -> Result<NodeId, Fail> {
    let id = OpId::parse(op).map_err(|_| unknown_op(ctx, op))?;
    if id.family().as_str() == INSTANCE_OP {
        return Err(fail(
            "group.instance_mismatch",
            "instances are made with add_instance, not add_node",
        ));
    }
    let spec = spec_of(ctx, &id)?;
    if ctx.doc.nodes.len() >= MAX_NODES {
        return Err(Box::new(
            Diagnostic::error("budget.nodes", "the document has too many nodes")
                .with_limit(count(MAX_NODES), count(ctx.doc.nodes.len() + 1))
                .suggest(format!(
                    "delete or group nodes to stay within {MAX_NODES} nodes"
                )),
        ));
    }
    if let Some(label) = label {
        text_ok(label, "the label")?;
    }
    let params = build_params(spec, params, None)?;
    let node_id = ctx
        .doc
        .issue_node_id()
        .ok_or_else(|| fail("budget.nodes", "node ids are used up"))?;
    ctx.doc.nodes.push(Node {
        id: node_id.clone(),
        op: id,
        label: label.map(str::to_owned),
        params,
        inputs: BTreeMap::new(),
        flags: Flags::default(),
        group: None,
        instance: None,
    });
    ctx.changed.insert(node_id.clone());
    Ok(node_id)
}

pub(crate) fn remove_node(ctx: &mut Ctx<'_>, node: &NodeId, cascade: bool) -> Result<(), Fail> {
    existing(ctx, node)?;
    let mut doomed: BTreeSet<NodeId> = BTreeSet::from([node.clone()]);
    loop {
        let readers: Vec<NodeId> = ctx
            .doc
            .nodes
            .iter()
            .filter(|n| !doomed.contains(&n.id))
            .filter(|n| {
                n.inputs
                    .values()
                    .flatten()
                    .any(|r| doomed.contains(&r.node))
            })
            .map(|n| n.id.clone())
            .collect();
        if readers.is_empty() {
            break;
        }
        if !cascade {
            return Err(Box::new(
                Diagnostic::error("graph.in_use", format!("{node} is still used"))
                    .at_node(node)
                    .with_related(readers)
                    .suggest("remove the readers first, or remove with cascade"),
            ));
        }
        doomed.extend(readers);
    }
    let named = ctx
        .doc
        .outputs
        .iter()
        .any(|o| doomed.contains(&o.from.node));
    if named && !cascade {
        return Err(Box::new(
            Diagnostic::error("graph.in_use", format!("{node} is a named output"))
                .at_node(node)
                .suggest("remove the output first, or remove with cascade"),
        ));
    }
    groups::guard_removal(ctx, &doomed)?;
    ctx.doc.nodes.retain(|n| !doomed.contains(&n.id));
    ctx.doc.outputs.retain(|o| !doomed.contains(&o.from.node));
    ctx.doc
        .editor
        .positions
        .retain(|id, _| !doomed.contains(id));
    ctx.doc.editor.collapsed.retain(|id| !doomed.contains(id));
    ctx.changed.extend(doomed);
    Ok(())
}

pub(crate) fn set_param(
    ctx: &mut Ctx<'_>,
    node: &NodeId,
    param: &str,
    value: &Value,
) -> Result<(), Fail> {
    let op = existing(ctx, node)?.op.clone();
    let spec = spec_of(ctx, &op)?;
    let param_spec = spec.param(param).ok_or_else(|| {
        let valid: Vec<&str> = spec.params.iter().map(|p| p.name).collect();
        Box::new(
            Diagnostic::error(
                "parse.unknown_param",
                format!("{op} has no parameter {param:?}"),
            )
            .at_node(node)
            .suggest(format!("parameters: {}", valid.join(", "))),
        )
    })?;
    let checked = param_spec
        .ty
        .check(value)
        .map_err(|issue| Box::new(Diagnostic::from_issue(issue, Some(node), param)))?;
    if let Some(n) = ctx.doc.node_mut(node) {
        n.params.insert(param.to_owned(), checked);
    }
    ctx.changed.insert(node.clone());
    Ok(())
}

pub(crate) fn set_label(ctx: &mut Ctx<'_>, node: &NodeId, label: Option<&str>) -> Result<(), Fail> {
    existing(ctx, node)?;
    if let Some(label) = label {
        text_ok(label, "the label")?;
    }
    if let Some(n) = ctx.doc.node_mut(node) {
        n.label = label.map(str::to_owned);
    }
    ctx.changed.insert(node.clone());
    Ok(())
}

pub(crate) fn set_flags(
    ctx: &mut Ctx<'_>,
    node: &NodeId,
    muted: Option<bool>,
    bypass: Option<bool>,
) -> Result<(), Fail> {
    existing(ctx, node)?;
    if let Some(n) = ctx.doc.node_mut(node) {
        n.flags.muted = muted.unwrap_or(n.flags.muted);
        n.flags.bypass = bypass.unwrap_or(n.flags.bypass);
    }
    ctx.changed.insert(node.clone());
    Ok(())
}

pub(crate) fn set_op_version(ctx: &mut Ctx<'_>, node: &NodeId, version: u32) -> Result<(), Fail> {
    let current = existing(ctx, node)?.clone();
    let target = current.op.with_version(version).map_err(|e| {
        Box::new(
            Diagnostic::error("parse.unknown_op", e.to_string())
                .at_node(node)
                .suggest(format!("use a version of at least 1, not {version}")),
        )
    })?;
    let spec = spec_of(ctx, &target)?;
    // Every existing parameter must still exist and be valid; new ones take their defaults.
    let params = build_params(spec, &current.params, Some(node))?;
    if let Some(port) = current.inputs.keys().find(|p| spec.input(p).is_none()) {
        let valid: Vec<&str> = spec.inputs.iter().map(|p| p.name).collect();
        let names = if valid.is_empty() {
            "none".to_owned()
        } else {
            valid.join(", ")
        };
        return Err(Box::new(
            Diagnostic::error(
                "graph.unknown_port",
                format!("{target} has no input {port:?}"),
            )
            .at_node(node)
            .at_port(port)
            .suggest(format!("{target} inputs: {names}")),
        ));
    }
    if let Some(n) = ctx.doc.node_mut(node) {
        n.op = target;
        n.params = params;
    }
    ctx.changed.insert(node.clone());
    links::check_graph(ctx)
}
