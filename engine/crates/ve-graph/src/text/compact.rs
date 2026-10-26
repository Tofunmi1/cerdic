//! Readable text with exact angle literals and conservatively folded pipe chains.

use std::collections::BTreeMap;

use crate::diag::Diagnostic;
use crate::doc::{Document, Node};
use crate::spec::OperatorRegistry;
use crate::value::{Unit, Value};

/// Prints a document with latest versions and default parameters omitted, short exact degrees,
/// and consecutive, single-use nodes named as pipe stages combined into one statement.
///
/// # Errors
/// The same unknown-operator, cycle and unsupported-instance diagnostics as [`super::to_text`].
pub fn to_text_compact(
    registry: &OperatorRegistry,
    doc: &Document,
) -> Result<String, Box<Diagnostic>> {
    super::print::render(registry, doc, true)
}

pub(super) fn literal(value: &Value, unit: Unit) -> String {
    match value {
        Value::Num(value) if unit == Unit::Angle => {
            let factor = std::f64::consts::PI / 180.0;
            for precision in 0..6 {
                let rounded = format!("{:.*e}", precision, value / factor);
                if let Ok(degrees) = rounded.parse::<f64>()
                    && (degrees * factor).to_bits() == value.to_bits()
                {
                    return format!("{degrees}deg");
                }
            }
            super::print::literal(&Value::Num(*value), unit)
        }
        Value::List(values) => format!(
            "[{}]",
            values
                .iter()
                .map(|v| literal(v, unit))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        _ => super::print::literal(value, unit),
    }
}

fn use_counts(doc: &Document) -> BTreeMap<&str, usize> {
    let mut counts = BTreeMap::new();
    for reference in doc.nodes.iter().flat_map(|n| n.inputs.values().flatten()) {
        *counts.entry(reference.node.as_str()).or_default() += 1;
    }
    for output in &doc.outputs {
        *counts.entry(output.from.node.as_str()).or_default() += 1;
    }
    for group in &doc.groups {
        *counts.entry(group.output.as_str()).or_default() += 1;
        for param in &group.params {
            *counts.entry(param.node.as_str()).or_default() += 1;
        }
    }
    counts
}

fn feeds(previous: &Node, next: &Node, registry: &OperatorRegistry) -> bool {
    if next.instance.is_some() {
        return false;
    }
    registry
        .get(&next.op)
        .and_then(|s| s.inputs.first())
        .and_then(|port| next.inputs.get(port.name))
        .and_then(|links| links.first())
        .is_some_and(|link| link.node == previous.id && link.port == "out")
}

/// `lines` and `nodes` share scope but may differ in order, since printing sorts dependencies.
pub(super) fn pipes(
    lines: Vec<String>,
    doc: &Document,
    registry: &OperatorRegistry,
    nodes: &[&Node],
    names: &BTreeMap<String, String>,
) -> Vec<String> {
    let counts = use_counts(doc);
    let by_name: BTreeMap<_, _> = nodes
        .iter()
        .filter_map(|n| names.get(n.id.as_str()).map(|name| (name.as_str(), *n)))
        .collect();
    let ordered: Vec<_> = lines
        .iter()
        .map(|line| {
            line.split_once(" = ")
                .and_then(|(name, _)| by_name.get(name).copied())
        })
        .collect();
    let mut out = Vec::new();
    let mut start = 0;
    while start < lines.len() {
        let mut best = start;
        for end in start + 1..lines.len() {
            let (Some(previous), Some(next)) = (ordered[end - 1], ordered[end]) else {
                break;
            };
            if counts.get(previous.id.as_str()) != Some(&1) || !feeds(previous, next, registry) {
                break;
            }
            let Some(root) = next.label.as_deref() else {
                continue;
            };
            if names.get(next.id.as_str()).is_none_or(|name| name != root) {
                continue;
            }
            if (start..end).all(|i| {
                ordered[i].is_some_and(|node| {
                    node.label.as_deref() == Some(format!("{root}_{}", i - start + 1).as_str())
                        && names.get(node.id.as_str()) == node.label.as_ref()
                })
            }) {
                best = end;
            }
        }
        if best == start {
            out.push(lines[start].clone());
            start += 1;
            continue;
        }
        let root = lines[best].split_once(" = ").map_or("", |(n, _)| n);
        let first = lines[start].split_once(" = ").map_or("", |(_, call)| call);
        let mut line = format!("{root} = {first}");
        for text in &lines[start + 1..=best] {
            let call = text.split_once(" = ").map_or("", |(_, call)| call);
            // The first argument is the first input (possibly named when later ports have gaps).
            if let Some((op, args)) = call.split_once('(') {
                let rest = args
                    .split_once(',')
                    .map_or(")", |(_, rest)| rest.trim_start());
                line.push_str(" | ");
                line.push_str(op);
                line.push('(');
                line.push_str(rest);
            }
        }
        out.push(line);
        start = best + 1;
    }
    out
}
