//! From statements to commands: resolve operators and names, then one batch for the whole text.

use std::collections::{BTreeMap, BTreeSet};

use super::group::{Block, lower_group};
use super::instance::{Groups, known_groups, lower_instance};
use super::lex::{Fail, error, fail};
use super::near;
use super::node::{Known, links, node_commands, node_ref};
use super::parse::{Arg, Stmt, parse};
use crate::command::{Command, OutRef};
use crate::diag::{Diagnostic, Span};
use crate::doc::Document;
use crate::id::NodeId;
use crate::spec::{OpId, OperatorRegistry, OperatorSpec, Readiness};

pub use super::batch::TextBatch;

pub(super) fn valid_name(name: &str) -> bool {
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

pub(super) fn resolve<'r>(
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
            let candidates: Vec<_> = registry
                .ready()
                .map(|s| {
                    if text.contains('@') {
                        s.id.to_string()
                    } else {
                        s.id.family().to_string()
                    }
                })
                .collect();
            let message = format!("unknown operator {text}");
            Err(Box::new(Diagnostic {
                suggestion: near::suggestion(text, candidates.iter().map(String::as_str), None),
                ..error("parse.unknown_op", span, message)
            }))
        }
    }
}

pub(super) struct Names<'a> {
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

pub(super) fn input_errors(
    statements: &[Stmt],
    names: &Names<'_>,
    known: &Known,
    out: &mut Vec<Diagnostic>,
) {
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
                None if known.contains_key(source.as_str()) => {}
                None => out.push(Diagnostic {
                    suggestion: near::suggestion(
                        source,
                        names
                            .defined
                            .iter()
                            .filter(|(_, at)| **at < index)
                            .map(|(n, _)| *n)
                            .chain(known.keys().map(String::as_str)),
                        None,
                    ),
                    ..error(
                        "parse.unknown_name",
                        *span,
                        format!("no node is named {source}"),
                    )
                }),
            }
        }
    }
}

/// Collects the node names and what each reads, reporting bad and duplicate names.
pub(super) fn collect_names<'a>(
    statements: &'a [Stmt],
    known: &Known,
    errors: &mut Vec<Diagnostic>,
) -> Names<'a> {
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
        } else if names.defined.contains_key(name.as_str()) || known.contains_key(name.as_str()) {
            // The first definition stays the one reads are checked against, so a read between two definitions is not
            // also reported as a forward reference.
            errors.push(error(
                if statements
                    .iter()
                    .any(|s| matches!(s, Stmt::Node { name: n, piped: true, .. } if n == name))
                {
                    "parse.name_taken"
                } else {
                    "parse.duplicate_name"
                },
                *name_span,
                format!("{name} is already the name of a node"),
            ));
        } else {
            names.defined.insert(name, index);
        }
        let reads = args.iter().filter_map(|a| match a {
            Arg::Input { name, .. } => Some(name.as_str()),
            Arg::Param { .. } => None,
        });
        names.deps.insert(name, reads.collect());
    }
    names
}

/// `output name` or `output name = node`: declares (or, for an existing name, redirects) an output.
fn output_command(
    name: &str,
    name_span: Span,
    node: Option<&(String, Span)>,
    names: &Names<'_>,
    known: &Known,
) -> Result<(Command, Span), Fail> {
    let (source, span) = node.map_or((name, name_span), |(n, s)| (n.as_str(), *s));
    let from = if names.defined.contains_key(source) || known.contains_key(source) {
        node_ref(source, known)
    } else {
        None
    };
    from.map(|from| {
        (
            Command::SetOutput {
                name: name.to_owned(),
                from: Some(OutRef::out(from)),
            },
            span,
        )
    })
    .ok_or_else(|| {
        Box::new(Diagnostic {
            suggestion: near::suggestion(
                source,
                names
                    .defined
                    .keys()
                    .copied()
                    .chain(known.keys().map(String::as_str)),
                None,
            ),
            ..error(
                "parse.unknown_name",
                span,
                format!("no node is named {source}"),
            )
        })
    })
}

/// Labels in `doc` that text can refer to: plain names that exactly one node carries.
fn known_labels(doc: &Document) -> Known {
    let mut seen: BTreeMap<String, Option<NodeId>> = BTreeMap::new();
    // The members of a group are not top-level nodes; text cannot read them.
    for node in doc.nodes.iter().filter(|n| n.group.is_none()) {
        let Some(label) = node.label.as_deref().filter(|l| valid_name(l)) else {
            continue;
        };
        seen.entry(label.to_owned())
            .and_modify(|slot| *slot = None)
            .or_insert_with(|| Some(node.id.clone()));
    }
    seen.into_iter()
        .filter_map(|(label, id)| id.map(|id| (label, id)))
        .collect()
}

/// Reads text into commands that create the described nodes, links and outputs.
///
/// # Errors
/// Every problem found, each with a source span: syntax, unknown operators (with suggestions),
/// parameters, names, forward references and cycles.
pub fn parse_text(registry: &OperatorRegistry, source: &str) -> Result<TextBatch, Vec<Diagnostic>> {
    parse_with(registry, &Known::new(), Groups::new(), source)
}

/// Like [`parse_text`], for adding to `doc`: a name the text does not define may be the label of one
/// of the document's nodes, which the new nodes then read; `output name = node` can redirect an
/// existing output; a new node may not reuse an existing label.
///
/// # Errors
/// The same problems as [`parse_text`], plus `parse.duplicate_name` for a reused label.
pub fn parse_text_in(
    registry: &OperatorRegistry,
    doc: &Document,
    source: &str,
) -> Result<TextBatch, Vec<Diagnostic>> {
    parse_with(
        registry,
        &known_labels(doc),
        known_groups(registry, doc),
        source,
    )
}

fn parse_with(
    registry: &OperatorRegistry,
    known: &Known,
    mut groups: Groups,
    source: &str,
) -> Result<TextBatch, Vec<Diagnostic>> {
    // Constants and `{expression}` values are folded into plain numbers before the parser sees the text.
    let expanded = super::repeat::expand(source).map_err(|e| vec![*e])?;
    let constants = &expanded.constants;
    let statements = parse(&expanded.text).map_err(|e| vec![expanded.locate(*e)])?;
    let mut errors = Vec::new();
    let names = collect_names(&statements, known, &mut errors);
    input_errors(&statements, &names, known, &mut errors);
    let mut batch = TextBatch::default();
    let mut serial = 0;
    for statement in &statements {
        match statement {
            // An operator is `family.name`; a name without a dot is a group being instanced.
            Stmt::Node {
                name,
                name_span,
                op,
                op_span,
                args,
                ..
            } if !op.contains('.') => {
                let (added, mut found) =
                    lower_instance(name, *name_span, op, *op_span, args, &groups, known);
                errors.append(&mut found);
                batch.extend(added);
            }
            Stmt::Node {
                name,
                name_span,
                op,
                op_span,
                args,
                ..
            } => match resolve(registry, op, *op_span) {
                Ok(spec) => {
                    let (added, mut e1) = node_commands(spec, name, args, *name_span);
                    let (linked, e2) = links(spec, name, args, known);
                    errors.append(&mut e1);
                    errors.extend(e2);
                    batch.extend(added);
                    batch.extend(linked);
                }
                Err(e) => errors.push(*e),
            },
            // aliases were replaced by the nodes they name when the statements were read
            Stmt::Alias { .. } => {}
            Stmt::Group {
                name,
                name_span,
                body,
                exposes,
                result,
            } => {
                let block = Block {
                    name,
                    name_span: *name_span,
                    body,
                    exposes,
                    result: result.as_ref(),
                };
                let (added, mut found) = lower_group(registry, &block, &mut groups, &mut serial);
                errors.append(&mut found);
                batch.extend(added);
            }
            Stmt::Output {
                name,
                name_span,
                node,
            } => match output_command(name, *name_span, node.as_ref(), &names, known) {
                Ok(item) => batch.extend(vec![item]),
                Err(e) => errors.push(*e),
            },
        }
    }
    super::expr::hint_constants(&mut errors, constants);
    batch.remap(&expanded);
    errors = errors.into_iter().map(|e| expanded.locate(e)).collect();
    errors.sort_by_key(|e| e.span.map(|s| (s.line, s.col)));
    if errors.is_empty() {
        Ok(batch)
    } else {
        Err(errors)
    }
}
