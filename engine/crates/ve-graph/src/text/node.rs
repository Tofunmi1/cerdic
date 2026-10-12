//! The commands one `name = op(...)` statement stands for: add the node, set flags, link inputs.

use std::collections::{BTreeMap, BTreeSet};

use super::coerce::{describe, param_value};
use super::lex::error;
use super::parse::{Arg, Lit};
use crate::command::{Alias, Command, InputRef, NodeRef, OutRef};
use crate::diag::{Diagnostic, Span};
use crate::id::NodeId;
use crate::spec::{Arity, OperatorSpec};
use crate::value::Value;

/// Commands with the span of the text they came from, and the problems found while building them.
pub(super) type Built = (Vec<(Command, Span)>, Vec<Diagnostic>);

const FLAGS: [&str; 2] = ["muted", "bypass"];

struct Params {
    values: BTreeMap<String, Value>,
    flags: BTreeMap<&'static str, bool>,
    bad: BTreeSet<String>,
    errors: Vec<Diagnostic>,
}

fn read_params(spec: &OperatorSpec, args: &[Arg]) -> Params {
    let mut out = Params {
        values: BTreeMap::new(),
        flags: BTreeMap::new(),
        bad: BTreeSet::new(),
        errors: Vec::new(),
    };
    for arg in args {
        let Arg::Param {
            key,
            key_span,
            value,
        } = arg
        else {
            continue;
        };
        if FLAGS.contains(&key.as_str()) {
            match value {
                Lit::Bool(b, _) => {
                    out.flags
                        .insert(if key == "muted" { "muted" } else { "bypass" }, *b);
                }
                other => {
                    out.errors
                        .push(error("param.type", other.span(), "expected true or false"));
                }
            }
        } else if let Some(param) = spec.param(key) {
            match param_value(&param.ty, key, value) {
                Ok(v) => {
                    out.values.insert(key.clone(), v);
                }
                Err(e) => {
                    out.bad.insert(key.clone());
                    out.errors.push(*e);
                }
            }
        } else {
            let valid: Vec<_> = spec.params.iter().map(|p| p.name).collect();
            out.errors.push(Diagnostic {
                suggestion: Some(format!("{} takes: {}", spec.id, valid.join(", "))),
                ..error(
                    "parse.unknown_param",
                    *key_span,
                    format!("{} has no parameter {key}", spec.id),
                )
            });
        }
    }
    out
}

/// `add_node` (and `set_flags` when `muted` or `bypass` were written) for one statement.
pub(super) fn node_commands(
    spec: &OperatorSpec,
    name: &str,
    args: &[Arg],
    name_span: Span,
) -> Built {
    let mut read = read_params(spec, args);
    for param in spec.params.iter().filter(|p| {
        p.default.is_none() && !read.values.contains_key(p.name) && !read.bad.contains(p.name)
    }) {
        read.errors.push(error(
            "parse.missing_param",
            name_span,
            format!(
                "{name}: {} is required: {}",
                param.name,
                describe(&param.ty)
            ),
        ));
    }
    let alias = Alias::parse(&format!("${name}")).ok();
    let mut commands = vec![(
        Command::AddNode {
            op: spec.id.to_string(),
            params: read.values,
            label: Some(name.to_owned()),
            alias: alias.clone(),
        },
        name_span,
    )];
    if let (false, Some(alias)) = (read.flags.is_empty(), alias) {
        commands.push((
            Command::SetFlags {
                node: NodeRef::Alias(alias),
                muted: read.flags.get("muted").copied(),
                bypass: read.flags.get("bypass").copied(),
            },
            name_span,
        ));
    }
    (commands, read.errors)
}

/// Nodes already in the document that the text may read, by label.
pub(super) type Known = BTreeMap<String, NodeId>;

/// A node the text refers to: one already in the document, or one this text defines.
pub(super) fn node_ref(name: &str, known: &Known) -> Option<NodeRef> {
    match known.get(name) {
        Some(id) => Some(NodeRef::Id(id.clone())),
        None => Alias::parse(&format!("${name}")).ok().map(NodeRef::Alias),
    }
}

fn connect(to: &str, port: &str, from: &str, known: &Known, span: Span) -> Option<(Command, Span)> {
    let to = Alias::parse(&format!("${to}")).ok()?;
    let from = node_ref(from, known)?;
    Some((
        Command::Connect {
            from: OutRef::out(from),
            to: InputRef {
                node: NodeRef::Alias(to),
                port: port.to_owned(),
                index: None,
            },
        },
        span,
    ))
}

/// `connect` commands for a statement's inputs: positional ones fill the ports in order (a
/// variadic port takes the rest), `port=name` ones go where they say.
pub(super) fn links(spec: &OperatorSpec, name: &str, args: &[Arg], known: &Known) -> Built {
    let mut errors = Vec::new();
    let mut commands = Vec::new();
    let positional: Vec<_> = args
        .iter()
        .filter_map(|a| match a {
            Arg::Input {
                port: None,
                name,
                span,
            } => Some((name, *span)),
            _ => None,
        })
        .collect();
    for arg in args {
        let Arg::Input {
            port: Some(port),
            name: source,
            span,
        } = arg
        else {
            continue;
        };
        if spec.input(port).is_some() {
            commands.extend(connect(name, port, source, known, *span));
        } else {
            errors.push(error(
                "graph.unknown_port",
                *span,
                format!("{} has no input {port}", spec.id),
            ));
        }
    }
    let mut next = 0;
    for port in &spec.inputs {
        let take = match port.arity {
            Arity::One => 1,
            Arity::Variadic { .. } => positional.len().saturating_sub(next),
        };
        for (source, span) in positional.iter().skip(next).take(take) {
            commands.extend(connect(name, port.name, source, known, *span));
        }
        next += take;
    }
    for (_, span) in positional.iter().skip(next) {
        errors.push(error(
            "parse.syntax",
            *span,
            format!("{} takes {} input(s)", spec.id, spec.inputs.len()),
        ));
    }
    (commands, errors)
}
