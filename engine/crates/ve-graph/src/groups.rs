//! Commands that create and change groups (ADR 0013); instances are in `groups/instances.rs`.

use std::collections::BTreeSet;

use crate::apply::{Ctx, Fail, fail};
use crate::diag::Diagnostic;
use crate::doc::Node;
use crate::group::{Group, GroupParam, INPUT_OP};
use crate::id::{GroupId, NodeId};
use crate::limits::{MAX_GROUP_PARAMS, MAX_GROUPS, count};
use crate::links;
use crate::nodes::unknown_node;
use crate::spec::PortType;

mod instances;

pub use instances::{UpgradePreview, upgrade_preview};
pub(crate) use instances::{
    add_instance, check_override, clear_instance_param, set_instance_param, upgrade_group,
};

fn unknown_group(id: &GroupId) -> Fail {
    Box::new(Diagnostic::error(
        "group.unknown",
        format!("there is no group {id}"),
    ))
}

fn group_of<'c>(ctx: &'c Ctx<'_>, id: &GroupId) -> Result<&'c Group, Fail> {
    ctx.doc.group(id).ok_or_else(|| unknown_group(id))
}

fn name_ok(name: &str, what: &str, longest: usize) -> Result<(), Fail> {
    if name.trim().is_empty() || name.len() > longest {
        return Err(Box::new(
            Diagnostic::error(
                "param.range",
                format!("{what} must be between 1 and {longest} bytes"),
            )
            .with_limit(count(longest), count(name.len())),
        ));
    }
    Ok(())
}

/// A group's name is how the text form refers to it, so it is a plain name (letters, digits and `_`,
/// not starting with a digit, at most 32 characters) that no other group has.
fn group_name_ok(ctx: &Ctx<'_>, name: &str, this: Option<&GroupId>) -> Result<(), Fail> {
    let mut chars = name.chars();
    let plain = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name.len() <= 32
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !plain {
        return Err(fail(
            "group.name",
            format!("{name:?} is not a usable group name (letters, digits, _; at most 32)"),
        ));
    }
    if ctx
        .doc
        .groups
        .iter()
        .any(|g| g.name == name && Some(&g.id) != this)
    {
        return Err(fail(
            "group.name",
            format!("another group is already called {name}"),
        ));
    }
    Ok(())
}

/// Increases a group's version: its interface changed, so existing instances are behind.
pub(crate) fn bump(ctx: &mut Ctx<'_>, id: &GroupId) {
    if let Some(group) = ctx.doc.group_mut(id) {
        group.version += 1;
    }
}

/// Makes a node just added a member of a group's definition. A new `group.input` changes the group's
/// interface, so it raises the group's version.
pub(crate) fn join(ctx: &mut Ctx<'_>, node: &NodeId, owner: &GroupId) -> Result<(), Fail> {
    group_of(ctx, owner)?;
    let mut is_input = false;
    if let Some(found) = ctx.doc.node_mut(node) {
        found.group = Some(owner.clone());
        is_input = found.op.family().as_str() == INPUT_OP;
    }
    if is_input {
        bump(ctx, owner);
    }
    Ok(())
}

/// The member `node` of `group`, or why it is not one.
fn member<'c>(ctx: &'c Ctx<'_>, group: &GroupId, node: &NodeId) -> Result<&'c Node, Fail> {
    let found = ctx.doc.node(node).ok_or_else(|| unknown_node(node))?;
    if found.group.as_ref() == Some(group) {
        Ok(found)
    } else {
        Err(Box::new(
            Diagnostic::error(
                "group.not_member",
                format!("{node} is not a member of {group}"),
            )
            .at_node(node),
        ))
    }
}

fn must_be_solid(ctx: &Ctx<'_>, node: &NodeId) -> Result<(), Fail> {
    match links::output_type(&ctx.doc, ctx.reg, node) {
        Some(PortType::Solid) => Ok(()),
        _ => Err(Box::new(
            Diagnostic::error("graph.port_type", "a group's result must be a solid").at_node(node),
        )),
    }
}

pub(crate) fn create_group(
    ctx: &mut Ctx<'_>,
    name: &str,
    members: &[NodeId],
    output: &NodeId,
) -> Result<GroupId, Fail> {
    group_name_ok(ctx, name, None)?;
    if ctx.doc.groups.len() >= MAX_GROUPS {
        return Err(Box::new(
            Diagnostic::error("budget.groups", "the document has too many groups")
                .with_limit(count(MAX_GROUPS), count(ctx.doc.groups.len() + 1)),
        ));
    }
    let unique: BTreeSet<&NodeId> = members.iter().collect();
    if unique.is_empty() || unique.len() != members.len() {
        return Err(fail(
            "group.interface",
            "a group needs at least one member, each listed once",
        ));
    }
    for id in members {
        let node = ctx.doc.node(id).ok_or_else(|| unknown_node(id))?;
        if let Some(owner) = &node.group {
            return Err(Box::new(
                Diagnostic::error(
                    "group.not_member",
                    format!("{id} already belongs to {owner}"),
                )
                .at_node(id),
            ));
        }
    }
    if !unique.contains(output) {
        return Err(Box::new(
            Diagnostic::error("group.interface", "the result must be one of the members")
                .at_node(output),
        ));
    }
    must_be_solid(ctx, output)?;
    let id = ctx
        .doc
        .issue_group_id()
        .ok_or_else(|| fail("budget.groups", "group ids are used up"))?;
    for node in ctx.doc.nodes.iter_mut().filter(|n| unique.contains(&n.id)) {
        node.group = Some(id.clone());
    }
    ctx.doc.groups.push(Group {
        id: id.clone(),
        name: name.to_owned(),
        version: 1,
        output: output.clone(),
        params: Vec::new(),
    });
    ctx.changed.extend(members.iter().cloned());
    Ok(id)
}

pub(crate) fn remove_group(ctx: &mut Ctx<'_>, id: &GroupId) -> Result<(), Fail> {
    group_of(ctx, id)?;
    let using: Vec<NodeId> = ctx.doc.instances_of(id).map(|n| n.id.clone()).collect();
    if !using.is_empty() {
        return Err(Box::new(
            Diagnostic::error("group.in_use", format!("{id} still has instances"))
                .with_related(using)
                .suggest("remove the instances first"),
        ));
    }
    let members: BTreeSet<NodeId> = ctx.doc.members(id).map(|n| n.id.clone()).collect();
    ctx.doc.nodes.retain(|n| !members.contains(&n.id));
    ctx.doc.groups.retain(|g| &g.id != id);
    ctx.doc.editor.positions.retain(|n, _| !members.contains(n));
    ctx.doc.editor.collapsed.retain(|n| !members.contains(n));
    ctx.changed.extend(members);
    Ok(())
}

pub(crate) fn set_group_name(ctx: &mut Ctx<'_>, id: &GroupId, name: &str) -> Result<(), Fail> {
    group_of(ctx, id)?;
    group_name_ok(ctx, name, Some(id))?;
    if let Some(group) = ctx.doc.group_mut(id) {
        name.clone_into(&mut group.name);
    }
    Ok(())
}

pub(crate) fn expose_param(
    ctx: &mut Ctx<'_>,
    id: &GroupId,
    name: &str,
    node: &NodeId,
    param: &str,
) -> Result<(), Fail> {
    name_ok(name, "the exposed name", 64)?;
    let group = group_of(ctx, id)?;
    let holder = member(ctx, id, node)?;
    let spec = ctx.reg.get(&holder.op).ok_or_else(|| {
        fail(
            "parse.unknown_op",
            format!("unknown operator {}", holder.op),
        )
    })?;
    if spec.param(param).is_none() {
        let valid: Vec<&str> = spec.params.iter().map(|p| p.name).collect();
        return Err(Box::new(
            Diagnostic::error(
                "parse.unknown_param",
                format!("{} has no parameter {param:?}", holder.op),
            )
            .at_node(node)
            .suggest(format!("parameters: {}", valid.join(", "))),
        ));
    }
    let clash = group
        .params
        .iter()
        .any(|p| p.name == name || (&p.node == node && p.param == param));
    if clash || group.params.len() >= MAX_GROUP_PARAMS {
        return Err(Box::new(
            Diagnostic::error(
                "group.interface",
                format!("{name:?} is already exposed, or that parameter is, or there are too many"),
            )
            .with_limit(count(MAX_GROUP_PARAMS), count(group.params.len() + 1)),
        ));
    }
    if let Some(group) = ctx.doc.group_mut(id) {
        group.params.push(GroupParam {
            name: name.to_owned(),
            node: node.clone(),
            param: param.to_owned(),
        });
    }
    bump(ctx, id);
    ctx.changed.insert(node.clone());
    Ok(())
}

pub(crate) fn unexpose_param(ctx: &mut Ctx<'_>, id: &GroupId, name: &str) -> Result<(), Fail> {
    let group = group_of(ctx, id)?;
    let Some(found) = group.params.iter().find(|p| p.name == name) else {
        let valid: Vec<&str> = group.params.iter().map(|p| p.name.as_str()).collect();
        return Err(Box::new(
            Diagnostic::error("group.unknown_param", format!("{name:?} is not exposed"))
                .suggest(format!("exposed: {}", valid.join(", "))),
        ));
    };
    ctx.changed.insert(found.node.clone());
    if let Some(group) = ctx.doc.group_mut(id) {
        group.params.retain(|p| p.name != name);
    }
    bump(ctx, id);
    Ok(())
}

pub(crate) fn set_group_output(ctx: &mut Ctx<'_>, id: &GroupId, node: &NodeId) -> Result<(), Fail> {
    group_of(ctx, id)?;
    member(ctx, id, node)?;
    must_be_solid(ctx, node)?;
    if let Some(group) = ctx.doc.group_mut(id)
        && &group.output != node
    {
        group.output = node.clone();
        bump(ctx, id);
    }
    ctx.changed.insert(node.clone());
    Ok(())
}

/// Whether removing `doomed` nodes would remove part of a group's interface; if so, which node and
/// why, so `remove_node` can refuse. A removed `group.input` bumps its group's version instead.
pub(crate) fn guard_removal(ctx: &mut Ctx<'_>, doomed: &BTreeSet<NodeId>) -> Result<(), Fail> {
    let mut bumped = BTreeSet::new();
    for id in doomed {
        let Some(node) = ctx.doc.node(id) else {
            continue;
        };
        let Some(group_id) = node.group.clone() else {
            continue;
        };
        let group = group_of(ctx, &group_id)?;
        let exposed = group.params.iter().any(|p| &p.node == id);
        if &group.output == id || exposed {
            let why = if exposed {
                "an exposed parameter"
            } else {
                "the group's result"
            };
            return Err(Box::new(
                Diagnostic::error("group.in_use", format!("{id} holds {why}"))
                    .at_node(id)
                    .suggest("change the interface first"),
            ));
        }
        if node.op.family().as_str() == INPUT_OP {
            bumped.insert(group_id);
        }
    }
    for group in bumped {
        bump(ctx, &group);
    }
    Ok(())
}
