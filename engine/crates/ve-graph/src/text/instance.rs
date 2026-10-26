//! Instance statements: `name = group(port=node, param=value)` (ADR 0013).
//!
//! An instance of a group written in this text or already in the document becomes `add_instance`, a
//! link for each group input written `port=node`, and an override for each parameter written with a
//! value. The group's version is never written: text always makes a current instance.

use std::collections::{BTreeMap, BTreeSet};

use super::coerce::param_value;
use super::lex::error;
use super::near;
use super::node::{Built, Known, connect};
use super::parse::{Arg, Lit};
use crate::command::{Alias, Command, GroupRef, NodeRef};
use crate::diag::{Diagnostic, Span};
use crate::doc::Document;
use crate::spec::OperatorRegistry;
use crate::value::ParamType;

/// What text can say about a group: how to refer to it and what an instance may bind or override.
pub(super) struct GroupInfo {
    pub(super) group: GroupRef,
    pub(super) params: BTreeMap<String, ParamType>,
    pub(super) inputs: BTreeSet<String>,
}

/// The groups text can instance, by name.
pub(super) type Groups = BTreeMap<String, GroupInfo>;

/// The groups already in `doc`.
pub(super) fn known_groups(registry: &OperatorRegistry, doc: &Document) -> Groups {
    doc.groups
        .iter()
        .map(|group| {
            let params = group
                .params
                .iter()
                .filter_map(|p| {
                    let ty = doc
                        .node(&p.node)
                        .and_then(|n| registry.get(&n.op))
                        .and_then(|spec| spec.param(&p.param))
                        .map(|param| param.ty)?;
                    Some((p.name.clone(), ty))
                })
                .collect();
            let inputs = doc
                .group_inputs(&group.id)
                .into_iter()
                .map(|i| i.name)
                .collect();
            (
                group.name.clone(),
                GroupInfo {
                    group: GroupRef::Id(group.id.clone()),
                    params,
                    inputs,
                },
            )
        })
        .collect()
}

pub(super) fn alias_of(key: &str) -> Option<Alias> {
    Alias::parse(&format!("${key}")).ok()
}

/// The statement being lowered.
struct Instance<'a> {
    name: &'a str,
    op: &'a str,
    info: &'a GroupInfo,
    alias: Option<Alias>,
    known: &'a Known,
}

/// What an argument contributed: commands, problems, and any `muted` / `bypass` it set.
#[derive(Default)]
struct Parts {
    commands: Vec<(Command, Span)>,
    errors: Vec<Diagnostic>,
    flags: BTreeMap<&'static str, bool>,
}

impl Instance<'_> {
    fn arg(&self, arg: &Arg, parts: &mut Parts) {
        match arg {
            Arg::Input {
                port: Some(port),
                name: source,
                span,
            } => self.bind(port, source, *span, parts),
            Arg::Input {
                port: None, span, ..
            } => parts.errors.push(error(
                "parse.syntax",
                *span,
                "a group's inputs are named: write port=node",
            )),
            Arg::Param {
                key,
                key_span,
                value,
            } => self.param(key, *key_span, value, parts),
        }
    }

    fn bind(&self, port: &str, source: &str, span: Span, parts: &mut Parts) {
        if self.info.inputs.contains(port) {
            parts
                .commands
                .extend(connect(self.name, port, source, self.known, span));
        } else {
            let valid: Vec<&str> = self.info.inputs.iter().map(String::as_str).collect();
            parts.errors.push(Diagnostic {
                suggestion: near::suggestion(
                    port,
                    valid.iter().copied(),
                    Some(format!("inputs: {}", valid.join(", "))),
                ),
                ..error(
                    "graph.unknown_port",
                    span,
                    format!("{} has no input {port}", self.op),
                )
            });
        }
    }

    fn param(&self, key: &str, key_span: Span, value: &Lit, parts: &mut Parts) {
        if key == "muted" || key == "bypass" {
            match value {
                Lit::Bool(b, _) => {
                    parts
                        .flags
                        .insert(if key == "muted" { "muted" } else { "bypass" }, *b);
                }
                other => {
                    parts
                        .errors
                        .push(error("param.type", other.span(), "expected true or false"));
                }
            }
            return;
        }
        let Some(ty) = self.info.params.get(key) else {
            let valid: Vec<&str> = self.info.params.keys().map(String::as_str).collect();
            parts.errors.push(Diagnostic {
                suggestion: near::suggestion(
                    key,
                    valid.iter().copied(),
                    Some(format!("{} exposes: {}", self.op, valid.join(", "))),
                ),
                ..error(
                    "parse.unknown_param",
                    key_span,
                    format!("{} does not expose {key}", self.op),
                )
            });
            return;
        };
        match (param_value(ty, key, value), self.alias.clone()) {
            (Ok(v), Some(alias)) => parts.commands.push((
                Command::SetInstanceParam {
                    node: NodeRef::Alias(alias),
                    param: key.to_owned(),
                    value: v,
                },
                key_span,
            )),
            (Err(e), _) => parts.errors.push(*e),
            (Ok(_), None) => {}
        }
    }
}

/// Lowers `name = group(...)`.
pub(super) fn lower_instance(
    name: &str,
    name_span: Span,
    op: &str,
    op_span: Span,
    args: &[Arg],
    groups: &Groups,
    known: &Known,
) -> Built {
    if op.contains('@') {
        return (
            Vec::new(),
            vec![error(
                "parse.group_version",
                op_span,
                "an instance is always of the group's current version; write the group's name without @",
            )],
        );
    }
    let Some(info) = groups.get(op) else {
        let all: Vec<&str> = groups.keys().map(String::as_str).collect();
        let hint = if all.is_empty() {
            "no group is defined; write `group name ... end` first".to_owned()
        } else {
            format!("groups: {}", all.join(", "))
        };
        return (
            Vec::new(),
            vec![Diagnostic {
                suggestion: Some(hint),
                ..error(
                    "parse.unknown_group",
                    op_span,
                    format!("no group is named {op}"),
                )
            }],
        );
    };
    let this = Instance {
        name,
        op,
        info,
        alias: alias_of(name),
        known,
    };
    let mut parts = Parts::default();
    parts.commands.push((
        Command::AddInstance {
            group: info.group.clone(),
            label: Some(name.to_owned()),
            alias: this.alias.clone(),
            into: None,
        },
        name_span,
    ));
    for arg in args {
        this.arg(arg, &mut parts);
    }
    if let (false, Some(alias)) = (parts.flags.is_empty(), this.alias.clone()) {
        parts.commands.push((
            Command::SetFlags {
                node: NodeRef::Alias(alias),
                muted: parts.flags.get("muted").copied(),
                bypass: parts.flags.get("bypass").copied(),
            },
            name_span,
        ));
    }
    (parts.commands, parts.errors)
}
