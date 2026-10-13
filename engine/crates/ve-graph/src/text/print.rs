//! Canonical text for a document: deterministic, and reading it back gives the same graph.

use std::collections::{BTreeMap, BTreeSet};

use crate::diag::Diagnostic;
use crate::doc::{Document, Node};
use crate::spec::{OperatorRegistry, OperatorSpec};
use crate::value::{ParamType, Unit, Value};

fn plain_name(label: &str) -> bool {
    let mut chars = label.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && label.len() <= 32
        && label != "output"
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A unique statement name per node: its label when usable, else its ID.
fn names(doc: &Document) -> BTreeMap<String, String> {
    let mut taken = BTreeSet::new();
    let mut out = BTreeMap::new();
    for node in &doc.nodes {
        let wanted = node.label.as_deref().filter(|l| plain_name(l));
        let name = match wanted {
            Some(l) if !taken.contains(l) && !doc.nodes.iter().any(|n| n.id.as_str() == l) => {
                l.to_owned()
            }
            _ => node.id.to_string(),
        };
        taken.insert(name.clone());
        out.insert(node.id.to_string(), name);
    }
    out
}

fn number(value: f64, unit: Unit) -> String {
    match unit {
        Unit::Angle => format!("{value}rad"),
        _ => format!("{value}"),
    }
}

fn literal(value: &Value, unit: Unit) -> String {
    match value {
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Num(n) => number(*n, unit),
        Value::Str(s) => format!(
            "\"{}\"",
            s.replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
        ),
        Value::List(items) => {
            let parts: Vec<_> = items.iter().map(|i| literal(i, unit)).collect();
            format!("[{}]", parts.join(", "))
        }
    }
}

fn unit_of(ty: &ParamType) -> Unit {
    match ty {
        ParamType::Num { unit, .. }
        | ParamType::Vec2 { unit, .. }
        | ParamType::Vec3 { unit, .. } => *unit,
        _ => Unit::None,
    }
}

fn inputs(node: &Node, spec: &OperatorSpec, names: &BTreeMap<String, String>) -> Vec<String> {
    let linked: Vec<Vec<&str>> = spec
        .inputs
        .iter()
        .map(|p| {
            node.inputs
                .get(p.name)
                .map(|links| {
                    links
                        .iter()
                        .filter_map(|l| names.get(l.node.as_str()).map(String::as_str))
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect();
    // Positional form needs the linked ports to form a prefix of the port list.
    let prefix = linked.iter().take_while(|l| !l.is_empty()).count();
    let positional = linked.iter().skip(prefix).all(Vec::is_empty);
    if positional {
        return linked.concat().into_iter().map(str::to_owned).collect();
    }
    spec.inputs
        .iter()
        .zip(&linked)
        .flat_map(|(port, nodes)| nodes.iter().map(move |n| format!("{}={n}", port.name)))
        .collect()
}

fn statement(node: &Node, spec: &OperatorSpec, names: &BTreeMap<String, String>) -> String {
    let mut args = inputs(node, spec, names);
    for param in &spec.params {
        let Some(value) = node.params.get(param.name) else {
            continue;
        };
        if param.default.as_ref() == Some(value) {
            continue;
        }
        args.push(format!(
            "{}={}",
            param.name,
            literal(value, unit_of(&param.ty))
        ));
    }
    if node.flags.muted {
        args.push("muted=true".to_owned());
    }
    if node.flags.bypass {
        args.push("bypass=true".to_owned());
    }
    let name = names.get(node.id.as_str()).map_or("", String::as_str);
    format!("{name} = {}({})", node.op, args.join(", "))
}

/// Prints the document as text, nodes in dependency order (document order among equals).
///
/// # Errors
/// `parse.unknown_op` if a node uses an operator the registry does not know, `graph.cycle` if the
/// links loop, and `text.unsupported` if the document has groups (the text form cannot yet say
/// them, and printing something that does not read back would be worse than refusing).
pub fn to_text(registry: &OperatorRegistry, doc: &Document) -> Result<String, Box<Diagnostic>> {
    if !doc.groups.is_empty()
        || doc
            .nodes
            .iter()
            .any(|n| n.group.is_some() || n.instance.is_some())
    {
        return Err(Box::new(
            Diagnostic::error(
                "text.unsupported",
                "this document has groups, which the text form cannot print yet",
            )
            .suggest("edit it in the panels, or with commands"),
        ));
    }
    let names = names(doc);
    let mut lines = Vec::new();
    let mut done: BTreeSet<&str> = BTreeSet::new();
    let mut pending: Vec<&Node> = doc.nodes.iter().collect();
    while !pending.is_empty() {
        let before = pending.len();
        let mut rest = Vec::new();
        for node in pending {
            let ready = node
                .inputs
                .values()
                .flatten()
                .all(|l| done.contains(l.node.as_str()));
            if !ready {
                rest.push(node);
                continue;
            }
            let spec = registry.get(&node.op).ok_or_else(|| {
                Box::new(
                    Diagnostic::error("parse.unknown_op", format!("{} is not registered", node.op))
                        .at_node(&node.id),
                )
            })?;
            lines.push(statement(node, spec, &names));
            done.insert(node.id.as_str());
        }
        if rest.len() == before {
            return Err(Box::new(Diagnostic::error(
                "graph.cycle",
                "the links form a cycle",
            )));
        }
        pending = rest;
    }
    for output in &doc.outputs {
        let source = names
            .get(output.from.node.as_str())
            .map_or("", String::as_str);
        lines.push(if source == output.name {
            format!("output {source}")
        } else {
            format!("output {} = {source}", output.name)
        });
    }
    lines.push(String::new());
    Ok(lines.join("\n"))
}
