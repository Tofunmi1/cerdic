//! Group blocks to commands (ADR 0013); instance statements are in `instance.rs`.
//!
//! A block `group name ... end` becomes: the members as top-level nodes with their links, then
//! `create_group` over them, then the group's inputs (which only exist inside a group) and the links
//! that read them, then `expose_param`. Members get internal aliases (`__serial_index`), so a member
//! can share a name with a top-level node or a member of another group; its label keeps the written
//! name.

use std::collections::{BTreeMap, BTreeSet};

use super::build::{Names, collect_names, input_errors, resolve, valid_name};
use super::instance::{GroupInfo, Groups, alias_of, lower_instance};
use super::lex::error;
use super::node::{Built, Known, links, node_commands};
use super::parse::{Arg, Expose, Lit, Stmt};
use crate::command::{Alias, Command, GroupRef, NodeRef, OutRef};
use crate::diag::{Diagnostic, Span};
use crate::group::INPUT_OP;
use crate::spec::{OperatorRegistry, OperatorSpec};
use crate::value::ParamType;

/// The arguments of a member statement with the nodes it reads renamed to their internal keys.
fn renamed(args: &[Arg], rename: &BTreeMap<String, String>) -> Vec<Arg> {
    args.iter()
        .map(|arg| match arg {
            Arg::Input { port, name, span } => Arg::Input {
                port: port.clone(),
                name: rename.get(name).cloned().unwrap_or_else(|| name.clone()),
                span: *span,
            },
            param @ Arg::Param { .. } => param.clone(),
        })
        .collect()
}

/// One written member of a group, lowered to commands.
struct Member<'s> {
    key: String,
    spec: Option<&'s OperatorSpec>,
    /// For a `group.input`, the name an instance links it by.
    input_name: Option<String>,
    commands: Vec<(Command, Span)>,
}

impl Member<'_> {
    fn is_input(&self) -> bool {
        self.spec
            .is_some_and(|s| s.id.family().as_str() == INPUT_OP)
    }
}

/// A `group` block as parsed.
pub(super) struct Block<'a> {
    pub(super) name: &'a str,
    pub(super) name_span: Span,
    pub(super) body: &'a [Stmt],
    pub(super) exposes: &'a [Expose],
    pub(super) result: Option<&'a (String, Span)>,
}

/// Puts the written name back where lowering used an internal key: the label of the node, and the
/// name in any message.
fn restore_name(
    commands: &mut [(Command, Span)],
    errors: &mut [Diagnostic],
    key: &str,
    name: &str,
) {
    let alias = format!("${key}");
    for (command, _) in commands {
        if let Command::AddNode {
            label, alias: a, ..
        }
        | Command::AddInstance {
            label, alias: a, ..
        } = command
            && a.as_ref().is_some_and(|a| a.as_str() == alias)
        {
            *label = Some(name.to_owned());
        }
    }
    for e in errors {
        e.message = e.message.replace(key, name);
    }
}

/// Lowers one member statement; `None` when it did not resolve (the error is recorded).
fn lower_member<'s>(
    registry: &'s OperatorRegistry,
    statement: &Stmt,
    rename: &BTreeMap<String, String>,
    groups: &Groups,
    errors: &mut Vec<Diagnostic>,
) -> Option<Member<'s>> {
    let Stmt::Node {
        name,
        name_span,
        op,
        op_span,
        args,
        ..
    } = statement
    else {
        return None;
    };
    let key = rename.get(name).cloned().unwrap_or_default();
    let args = renamed(args, rename);
    let known = Known::new();
    let (mut commands, mut found, spec) = if op.contains('.') {
        let spec = match resolve(registry, op, *op_span) {
            Ok(spec) => spec,
            Err(e) => {
                errors.push(*e);
                return None;
            }
        };
        let (mut added, e1) = node_commands(spec, &key, &args, *name_span);
        let (linked, e2) = links(spec, &key, &args, &known);
        added.extend(linked);
        (added, [e1, e2].concat(), Some(spec))
    } else {
        let (commands, found) =
            lower_instance(&key, *name_span, op, *op_span, &args, groups, &known);
        (commands, found, None)
    };
    restore_name(&mut commands, &mut found, &key, name);
    errors.append(&mut found);
    let input_name = spec
        .filter(|s| s.id.family().as_str() == INPUT_OP)
        .and_then(|_| {
            args.iter().find_map(|arg| match arg {
                Arg::Param {
                    key,
                    value: Lit::Text(text, _),
                    ..
                } if key == "name" => Some(text.clone()),
                _ => None,
            })
        });
    Some(Member {
        key,
        spec,
        input_name,
        commands,
    })
}

/// Problems with the group's name or its missing result line.
fn header_errors(block: &Block<'_>, groups: &Groups) -> Vec<Diagnostic> {
    let mut errors = Vec::new();
    if !valid_name(block.name) {
        errors.push(error(
            "parse.syntax",
            block.name_span,
            format!(
                "{:?} is not a usable group name (letters, digits, _; at most 32)",
                block.name
            ),
        ));
    } else if groups.contains_key(block.name) {
        errors.push(error(
            "parse.duplicate_name",
            block.name_span,
            format!("{} is already the name of a group", block.name),
        ));
    }
    if block.result.is_none() {
        errors.push(error(
            "parse.missing_result",
            block.name_span,
            format!(
                "group {} needs a `result` line naming the member that is its result",
                block.name
            ),
        ));
    }
    errors
}

/// Problems with the member named as the result, and with a group of nothing but inputs.
fn result_errors(
    block: &Block<'_>,
    result: &(String, Span),
    rename: &BTreeMap<String, String>,
    members: &[Member<'_>],
) -> Vec<Diagnostic> {
    let mut errors = Vec::new();
    let key = rename.get(&result.0);
    match key.and_then(|k| members.iter().find(|m| &m.key == k)) {
        None if key.is_none() => errors.push(error(
            "parse.unknown_name",
            result.1,
            format!("no member of group {} is named {}", block.name, result.0),
        )),
        Some(m) if m.is_input() => errors.push(error(
            "parse.syntax",
            result.1,
            "a group input cannot be the group's result",
        )),
        _ => {}
    }
    if members.iter().all(Member::is_input) {
        errors.push(error(
            "parse.syntax",
            block.name_span,
            format!(
                "group {} needs at least one member besides its inputs",
                block.name
            ),
        ));
    }
    errors
}

/// The commands in an order that is valid to apply: the members and the links among them, the group,
/// its inputs, the links that read the inputs, then the exposed parameters.
fn ordered(
    block: &Block<'_>,
    members: &[Member<'_>],
    group: &Alias,
    result: Alias,
    exposed: Vec<(Command, Span)>,
) -> Vec<(Command, Span)> {
    let input_keys: BTreeSet<&str> = members
        .iter()
        .filter(|m| m.is_input())
        .map(|m| m.key.as_str())
        .collect();
    // A link that reads a group input waits until the input exists, which is after the group does.
    let reads_input = |command: &Command| {
        matches!(command, Command::Connect { from: OutRef { node: NodeRef::Alias(a), .. }, .. }
            if input_keys.contains(&a.as_str()[1..]))
    };
    let (mut out, mut later) = (Vec::new(), Vec::new());
    for member in members.iter().filter(|m| !m.is_input()) {
        for (command, span) in &member.commands {
            let side = if reads_input(command) {
                &mut later
            } else {
                &mut out
            };
            side.push((command.clone(), *span));
        }
    }
    out.push((
        Command::CreateGroup {
            name: block.name.to_owned(),
            members: members
                .iter()
                .filter(|m| !m.is_input())
                .filter_map(|m| alias_of(&m.key).map(NodeRef::Alias))
                .collect(),
            output: NodeRef::Alias(result),
            alias: Some(group.clone()),
        },
        block.name_span,
    ));
    for member in members.iter().filter(|m| m.is_input()) {
        for (command, span) in &member.commands {
            let mut command = command.clone();
            if let Command::AddNode { group: g, .. } = &mut command {
                *g = Some(GroupRef::Alias(group.clone()));
            }
            out.push((command, *span));
        }
    }
    out.extend(later);
    out.extend(exposed);
    out
}

/// Lowers a `group` block; registers the group in `groups` so later statements can instance it.
pub(super) fn lower_group<'r>(
    registry: &'r OperatorRegistry,
    block: &Block<'_>,
    groups: &mut Groups,
    serial: &mut usize,
) -> Built {
    let mut errors = header_errors(block, groups);
    let Some(result) = block.result else {
        return (Vec::new(), errors);
    };
    let known = Known::new();
    let names: Names<'_> = collect_names(block.body, &known, &mut errors);
    input_errors(block.body, &names, &known, &mut errors);
    *serial += 1;
    let scope = *serial;
    let rename: BTreeMap<String, String> = block
        .body
        .iter()
        .filter_map(|s| match s {
            Stmt::Node { name, .. } => Some(name.clone()),
            _ => None,
        })
        .enumerate()
        .map(|(i, name)| (name, format!("__{scope}_{i}")))
        .collect();
    let members: Vec<Member<'r>> = block
        .body
        .iter()
        .filter_map(|s| lower_member(registry, s, &rename, groups, &mut errors))
        .collect();
    errors.extend(result_errors(block, result, &rename, &members));
    let group = alias_of(&format!("__g{scope}"));
    let (exposed, expose_errors) = lower_exposes(block, &rename, &members, group.as_ref());
    errors.extend(expose_errors);
    let result_alias = rename.get(&result.0).map(String::as_str).and_then(alias_of);
    let (Some(group), Some(result_alias)) = (group, result_alias) else {
        return (Vec::new(), errors);
    };
    if !errors.is_empty() {
        return (Vec::new(), errors);
    }
    let inputs = members
        .iter()
        .filter_map(|m| m.input_name.clone())
        .collect();
    let commands = ordered(block, &members, &group, result_alias, exposed.commands);
    groups.insert(
        block.name.to_owned(),
        GroupInfo {
            group: GroupRef::Alias(group),
            params: exposed.types,
            inputs,
        },
    );
    (commands, errors)
}

struct Exposed {
    commands: Vec<(Command, Span)>,
    types: BTreeMap<String, ParamType>,
}

/// `expose` lines to `expose_param` commands, checking the member and its parameter.
fn lower_exposes(
    block: &Block<'_>,
    rename: &BTreeMap<String, String>,
    members: &[Member<'_>],
    group: Option<&Alias>,
) -> (Exposed, Vec<Diagnostic>) {
    let mut out = Exposed {
        commands: Vec::new(),
        types: BTreeMap::new(),
    };
    let mut errors = Vec::new();
    for expose in block.exposes {
        let member = rename
            .get(&expose.node)
            .and_then(|key| members.iter().find(|m| &m.key == key));
        let Some(member) = member else {
            errors.push(error(
                "parse.unknown_name",
                expose.span,
                format!("no member of group {} is named {}", block.name, expose.node),
            ));
            continue;
        };
        let Some(param) = member.spec.and_then(|s| s.param(&expose.param)) else {
            errors.push(error(
                "parse.unknown_param",
                expose.span,
                format!("{} has no parameter {}", expose.node, expose.param),
            ));
            continue;
        };
        if out.types.insert(expose.name.clone(), param.ty).is_some() {
            errors.push(error(
                "parse.duplicate_name",
                expose.name_span,
                format!("{} is exposed twice", expose.name),
            ));
            continue;
        }
        if let (Some(group), Some(node)) = (group, alias_of(&member.key)) {
            out.commands.push((
                Command::ExposeParam {
                    group: GroupRef::Alias(group.clone()),
                    name: expose.name.clone(),
                    node: NodeRef::Alias(node),
                    param: expose.param.clone(),
                },
                expose.span,
            ));
        }
    }
    (out, errors)
}
