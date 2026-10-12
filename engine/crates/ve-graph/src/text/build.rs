//! From statements to commands: resolve operators and names, then one batch for the whole text.

use std::collections::{BTreeMap, BTreeSet};

use super::lex::{Fail, error, fail};
use super::node::{links, node_commands};
use super::parse::{Arg, Stmt, parse};
use crate::command::{Alias, Command, NodeRef, OutRef};
use crate::diag::{Diagnostic, Span};
use crate::spec::{OpId, OperatorRegistry, OperatorSpec, Readiness};

/// Commands read from text, with the source span each one came from.
#[derive(Clone, Debug)]
pub struct TextBatch {
    /// The commands, in an order that is valid to apply.
    pub commands: Vec<Command>,
    /// For each command, where in the text it was written.
    pub spans: Vec<Span>,
}

impl TextBatch {
    /// Adds the source span to a diagnostic that names a command of this batch.
    #[must_use]
    pub fn locate(&self, diagnostic: Diagnostic) -> Diagnostic {
        match diagnostic.command.and_then(|i| self.spans.get(i)) {
            Some(span) => diagnostic.at_span(*span),
            None => diagnostic,
        }
    }
}

fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name.len() <= 32
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn latest(registry: &OperatorRegistry, family: &str) -> Option<OpId> {
    registry
        .iter()
        .filter(|s| s.id.family().as_str() == family)
        .map(|s| s.id.clone())
        .max_by_key(OpId::version)
}

fn resolve<'r>(
    registry: &'r OperatorRegistry,
    text: &str,
    span: Span,
) -> Result<&'r OperatorSpec, Fail> {
    let id = if text.contains('@') {
        OpId::parse(text).ok()
    } else {
        latest(registry, text)
    };
    match id.as_ref().and_then(|id| registry.get(id)) {
        Some(spec) if spec.readiness == Readiness::Ready => Ok(spec),
        Some(spec) => Err(fail(
            "parse.unknown_op",
            span,
            format!(
                "{} is a draft operator and cannot be used from text yet",
                spec.id
            ),
        )),
        None => {
            let near = registry.suggest(&if text.contains('@') {
                text.to_owned()
            } else {
                format!("{text}@1")
            });
            let message = format!("unknown operator {text}");
            Err(Box::new(Diagnostic {
                suggestion: (!near.is_empty())
                    .then(|| format!("did you mean {}?", near.join(" or "))),
                ..error("parse.unknown_op", span, message)
            }))
        }
    }
}

struct Names<'a> {
    defined: BTreeMap<&'a str, usize>,
    deps: BTreeMap<&'a str, Vec<&'a str>>,
}

fn reaches(
    deps: &BTreeMap<&str, Vec<&str>>,
    from: &str,
    target: &str,
    seen: &mut BTreeSet<String>,
) -> bool {
    if !seen.insert(from.to_owned()) {
        return false;
    }
    deps.get(from)
        .into_iter()
        .flatten()
        .any(|d| *d == target || reaches(deps, d, target, seen))
}

fn input_errors(statements: &[Stmt], names: &Names<'_>, out: &mut Vec<Diagnostic>) {
    for (index, statement) in statements.iter().enumerate() {
        let Stmt::Node { name, args, .. } = statement else {
            continue;
        };
        for arg in args {
            let Arg::Input {
                name: source, span, ..
            } = arg
            else {
                continue;
            };
            match names.defined.get(source.as_str()) {
                Some(at) if *at < index => {}
                Some(_) => {
                    let cycle =
                        reaches(&names.deps, source, name, &mut BTreeSet::new()) || source == name;
                    let (code, why) = if cycle {
                        ("graph.cycle", "part of a cycle with it")
                    } else {
                        (
                            "parse.forward_reference",
                            "defined later; define inputs first",
                        )
                    };
                    out.push(error(
                        code,
                        *span,
                        format!("{name} reads {source}, which is {why}"),
                    ));
                }
                None => out.push(error(
                    "parse.unknown_name",
                    *span,
                    format!("no node is named {source}"),
                )),
            }
        }
    }
}

/// Collects the node names and what each reads, reporting bad and duplicate names.
fn collect_names<'a>(statements: &'a [Stmt], errors: &mut Vec<Diagnostic>) -> Names<'a> {
    let mut names = Names {
        defined: BTreeMap::new(),
        deps: BTreeMap::new(),
    };
    for (index, statement) in statements.iter().enumerate() {
        let Stmt::Node {
            name,
            name_span,
            args,
            ..
        } = statement
        else {
            continue;
        };
        if !valid_name(name) || name == "output" {
            errors.push(error(
                "parse.syntax",
                *name_span,
                format!("{name:?} is not a usable node name (letters, digits, _; at most 32)"),
            ));
        } else if names.defined.insert(name, index).is_some() {
            errors.push(error(
                "parse.duplicate_name",
                *name_span,
                format!("{name} is defined twice"),
            ));
        }
        let reads = args.iter().filter_map(|a| match a {
            Arg::Input { name, .. } => Some(name.as_str()),
            Arg::Param { .. } => None,
        });
        names.deps.insert(name, reads.collect());
    }
    names
}

/// Reads text into commands that create the described nodes, links and outputs.
///
/// # Errors
/// Every problem found, each with a source span: syntax, unknown operators (with suggestions),
/// parameters, names, forward references and cycles.
pub fn parse_text(registry: &OperatorRegistry, source: &str) -> Result<TextBatch, Vec<Diagnostic>> {
    let statements = parse(source).map_err(|e| vec![*e])?;
    let mut errors = Vec::new();
    let names = collect_names(&statements, &mut errors);
    input_errors(&statements, &names, &mut errors);
    let mut batch = TextBatch {
        commands: Vec::new(),
        spans: Vec::new(),
    };
    let mut push = |items: Vec<(Command, Span)>| {
        for (command, span) in items {
            batch.commands.push(command);
            batch.spans.push(span);
        }
    };
    for statement in &statements {
        match statement {
            Stmt::Node {
                name,
                name_span,
                op,
                op_span,
                args,
            } => match resolve(registry, op, *op_span) {
                Ok(spec) => {
                    let (added, mut e1) = node_commands(spec, name, args, *name_span);
                    let (linked, e2) = links(spec, name, args);
                    errors.append(&mut e1);
                    errors.extend(e2);
                    push(added);
                    push(linked);
                }
                Err(e) => errors.push(*e),
            },
            Stmt::Output {
                name,
                name_span,
                node,
            } => {
                let (source, span) = node
                    .as_ref()
                    .map_or((name.as_str(), *name_span), |(n, s)| (n.as_str(), *s));
                match Alias::parse(&format!("${source}")) {
                    Ok(alias) if names.defined.contains_key(source) => push(vec![(
                        Command::SetOutput {
                            name: name.clone(),
                            from: Some(OutRef::out(NodeRef::Alias(alias))),
                        },
                        span,
                    )]),
                    _ => errors.push(error(
                        "parse.unknown_name",
                        span,
                        format!("no node is named {source}"),
                    )),
                }
            }
        }
    }
    errors.sort_by_key(|e| e.span.map(|s| (s.line, s.col)));
    if errors.is_empty() {
        Ok(batch)
    } else {
        Err(errors)
    }
}
