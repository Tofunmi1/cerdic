//! The rules a document with groups must always satisfy. They are checked after every command, so a
//! rule is broken by no command and no batch, whichever command would have broken it.

use std::collections::{BTreeMap, BTreeSet};

use crate::apply::{Ctx, Fail};
use crate::diag::Diagnostic;
use crate::doc::Document;
use crate::group::{INPUT_OP, INSTANCE_OP};
use crate::groups::check_override;
use crate::id::{GroupId, NodeId};
use crate::limits::{MAX_GROUP_DEPTH, MAX_GROUP_PARAMS, count};
use crate::links;
use crate::spec::PortType;

fn error(code: &str, message: impl Into<String>, node: &NodeId) -> Fail {
    Box::new(Diagnostic::error(code, message).at_node(node))
}

/// Checks every group rule; cheap when the document has no groups.
pub(crate) fn check(ctx: &Ctx<'_>) -> Result<(), Fail> {
    let doc = &ctx.doc;
    let uses_groups = !doc.groups.is_empty()
        || doc.nodes.iter().any(|n| {
            n.group.is_some()
                || n.instance.is_some()
                || n.op.family().as_str().starts_with("group.")
        });
    if !uses_groups {
        return Ok(());
    }
    nodes(doc)?;
    scopes(doc)?;
    interfaces(ctx)?;
    cycles_and_depth(doc)?;
    overrides(ctx)
}

/// The rules that need no operator registry: what evaluation relies on to unfold a loaded document
/// safely (known groups, scopes, no recursion, bounded depth).
pub(crate) fn structure(doc: &Document) -> Result<(), Fail> {
    nodes(doc)?;
    scopes(doc)?;
    cycles_and_depth(doc)
}

/// Each node is what its operator and its group fields say it is.
fn nodes(doc: &Document) -> Result<(), Fail> {
    for node in &doc.nodes {
        let family = node.op.family().as_str();
        if family == INPUT_OP && node.group.is_none() {
            return Err(error(
                "group.input_outside",
                "a group input only makes sense inside a group",
                &node.id,
            ));
        }
        if (family == INSTANCE_OP) != node.instance.is_some() {
            return Err(error(
                "group.instance_mismatch",
                "instances are made with add_instance, and only they use group.use",
                &node.id,
            ));
        }
        if let Some(owner) = &node.group
            && doc.group(owner).is_none()
        {
            return Err(error(
                "group.unknown",
                format!("{} belongs to {owner}, which does not exist", node.id),
                &node.id,
            ));
        }
        if let Some(instance) = &node.instance {
            let Some(group) = doc.group(&instance.group) else {
                return Err(error(
                    "group.unknown",
                    format!(
                        "{} instances {}, which does not exist",
                        node.id, instance.group
                    ),
                    &node.id,
                ));
            };
            if instance.version > group.version {
                return Err(error(
                    "group.version",
                    "an instance cannot be newer than its group",
                    &node.id,
                ));
            }
        }
    }
    Ok(())
}

/// Links and outputs stay inside one scope: the top level, or one group's definition.
fn scopes(doc: &Document) -> Result<(), Fail> {
    for node in &doc.nodes {
        for source in node.inputs.values().flatten() {
            let Some(from) = doc.node(&source.node) else {
                continue; // a dangling link is the graph checker's to report
            };
            if from.group != node.group {
                return Err(Box::new(
                    Diagnostic::error(
                        "group.leaks",
                        "a link may not cross the edge of a group; use a group input or an instance",
                    )
                    .at_node(&node.id)
                    .with_related(vec![from.id.clone()])
                    .suggest("route the value through a group.input or an instance"),
                ));
            }
        }
    }
    for output in &doc.outputs {
        if doc
            .node(&output.from.node)
            .is_some_and(|n| n.group.is_some())
        {
            return Err(error(
                "group.leaks",
                "a member of a group cannot be a named output; output an instance",
                &output.from.node,
            ));
        }
    }
    Ok(())
}

/// Each group's output, exposed parameters and inputs are valid members.
fn interfaces(ctx: &Ctx<'_>) -> Result<(), Fail> {
    let doc = &ctx.doc;
    for group in &doc.groups {
        let Some(output) = doc.node(&group.output) else {
            return Err(error(
                "group.interface",
                "the group's result is gone",
                &group.output,
            ));
        };
        if output.group.as_ref() != Some(&group.id) {
            return Err(error(
                "group.interface",
                "the group's result must be one of its members",
                &output.id,
            ));
        }
        if links::output_type(doc, ctx.reg, &output.id) != Some(PortType::Solid) {
            return Err(Box::new(
                Diagnostic::error("graph.port_type", "a group's result must be a solid")
                    .at_node(&output.id)
                    .suggest("make the group's result a solid, for example solid.box"),
            ));
        }
        let mut names = BTreeSet::new();
        if group.params.len() > MAX_GROUP_PARAMS {
            return Err(error(
                "group.interface",
                "too many exposed parameters",
                &output.id,
            ));
        }
        for param in &group.params {
            let holder = doc
                .node(&param.node)
                .filter(|n| n.group.as_ref() == Some(&group.id));
            let known = holder.is_some_and(|n| {
                ctx.reg
                    .get(&n.op)
                    .is_some_and(|spec| spec.param(&param.param).is_some())
            });
            if !known || !names.insert(param.name.as_str()) {
                return Err(error(
                    "group.interface",
                    format!(
                        "the exposed parameter {:?} is invalid or repeated",
                        param.name
                    ),
                    &param.node,
                ));
            }
        }
        let mut inputs = BTreeSet::new();
        for input in doc.group_inputs(&group.id) {
            if input.name.trim().is_empty() || !inputs.insert(input.name.clone()) {
                return Err(error(
                    "group.interface",
                    format!(
                        "group input names must be unique and not empty: {:?}",
                        input.name
                    ),
                    &input.node,
                ));
            }
        }
    }
    Ok(())
}

/// Instances that sit inside a group's definition make the group graph; it must have no cycle and
/// stay shallow.
fn cycles_and_depth(doc: &Document) -> Result<(), Fail> {
    // edges: group -> (group instanced inside it, the instance node)
    let mut edges: BTreeMap<&GroupId, Vec<(&GroupId, &NodeId)>> = BTreeMap::new();
    for node in &doc.nodes {
        if let (Some(owner), Some(instance)) = (&node.group, &node.instance) {
            edges
                .entry(owner)
                .or_default()
                .push((&instance.group, &node.id));
        }
    }
    for node in &doc.nodes {
        if let (Some(owner), Some(instance)) = (&node.group, &node.instance)
            && reaches(&edges, &instance.group, owner, &mut BTreeSet::new())
        {
            return Err(Box::new(
                Diagnostic::error(
                    "group.recursive",
                    format!("{owner} would contain itself through {}", instance.group),
                )
                .at_node(&node.id)
                .suggest("remove the instance that closes the loop"),
            ));
        }
    }
    let deepest = doc
        .groups
        .iter()
        .map(|g| depth(&edges, &g.id, 0))
        .max()
        .unwrap_or(0);
    if deepest > MAX_GROUP_DEPTH {
        return Err(Box::new(
            Diagnostic::error("budget.group_depth", "groups are nested too deeply")
                .with_limit(count(MAX_GROUP_DEPTH), count(deepest))
                .suggest("flatten the group nesting"),
        ));
    }
    Ok(())
}

/// Whether `from` can reach `target` through nested instances (including `from == target`).
fn reaches(
    edges: &BTreeMap<&GroupId, Vec<(&GroupId, &NodeId)>>,
    from: &GroupId,
    target: &GroupId,
    seen: &mut BTreeSet<GroupId>,
) -> bool {
    if from == target {
        return true;
    }
    if !seen.insert(from.clone()) {
        return false;
    }
    edges
        .get(from)
        .is_some_and(|next| next.iter().any(|(g, _)| reaches(edges, g, target, seen)))
}

/// Levels of nesting below `group`: a group with no nested instances has depth 1. A cycle was already
/// refused, so this ends.
fn depth(
    edges: &BTreeMap<&GroupId, Vec<(&GroupId, &NodeId)>>,
    group: &GroupId,
    level: usize,
) -> usize {
    if level > MAX_GROUP_DEPTH + 1 {
        return level;
    }
    edges.get(group).map_or(level + 1, |next| {
        next.iter()
            .map(|(g, _)| depth(edges, g, level + 1))
            .max()
            .unwrap_or(level + 1)
    })
}

/// Overrides on current-version instances name exposed parameters and hold fitting values. An
/// instance that is behind is not checked: it is waiting for `upgrade_group`.
fn overrides(ctx: &Ctx<'_>) -> Result<(), Fail> {
    for node in &ctx.doc.nodes {
        let Some(instance) = &node.instance else {
            continue;
        };
        let Some(group) = ctx.doc.group(&instance.group) else {
            continue;
        };
        if instance.version != group.version {
            continue;
        }
        for (name, value) in &instance.overrides {
            let Some(exposed) = group.params.iter().find(|p| &p.name == name) else {
                return Err(error(
                    "group.unknown_param",
                    format!("{name:?} is not exposed by the group"),
                    &node.id,
                ));
            };
            check_override(ctx, exposed, value, &node.id)?;
        }
    }
    Ok(())
}
