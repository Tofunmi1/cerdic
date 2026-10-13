//! Canonical text for a document: deterministic, and reading it back gives the same graph.

use std::collections::{BTreeMap, BTreeSet};

use crate::diag::Diagnostic;
use crate::doc::{Document, Node};
use crate::group::{Group, Instance};
use crate::id::GroupId;
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

/// A unique statement name per node of one scope (the top level, or one group): its label when
/// usable, else its ID.
fn names(nodes: &[&Node]) -> BTreeMap<String, String> {
    let mut taken = BTreeSet::new();
    let mut out = BTreeMap::new();
    for node in nodes {
        let wanted = node.label.as_deref().filter(|l| plain_name(l));
        let name = match wanted {
            Some(l) if !taken.contains(l) && !nodes.iter().any(|n| n.id.as_str() == l) => {
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

fn flag_args(node: &Node, args: &mut Vec<String>) {
    if node.flags.muted {
        args.push("muted=true".to_owned());
    }
    if node.flags.bypass {
        args.push("bypass=true".to_owned());
    }
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
    flag_args(node, &mut args);
    let name = names.get(node.id.as_str()).map_or("", String::as_str);
    format!("{name} = {}({})", node.op, args.join(", "))
}

fn unsupported(message: String) -> Box<Diagnostic> {
    Box::new(Diagnostic::error("text.unsupported", message).suggest("upgrade it, then print again"))
}

/// `name = group(port=node, param=value)`; an instance behind its group cannot be written, because
/// text always makes a current instance.
fn instance_statement(
    doc: &Document,
    registry: &OperatorRegistry,
    node: &Node,
    instance: &Instance,
    names: &BTreeMap<String, String>,
) -> Result<String, Box<Diagnostic>> {
    let group = doc.group(&instance.group).ok_or_else(|| {
        Box::new(
            Diagnostic::error("group.unknown", "an instance names a missing group")
                .at_node(&node.id),
        )
    })?;
    if instance.version != group.version {
        return Err(unsupported(format!(
            "{} is behind its group {} (version {} of {}); upgrade it first",
            names
                .get(node.id.as_str())
                .map_or("an instance", String::as_str),
            group.name,
            instance.version,
            group.version
        )));
    }
    let mut args: Vec<String> = node
        .inputs
        .iter()
        .flat_map(|(port, links)| {
            links
                .iter()
                .filter_map(move |l| names.get(l.node.as_str()).map(|n| format!("{port}={n}")))
        })
        .collect();
    for (name, value) in &instance.overrides {
        let unit = group
            .params
            .iter()
            .find(|p| &p.name == name)
            .and_then(|p| {
                let holder = doc.node(&p.node)?;
                Some(unit_of(&registry.get(&holder.op)?.param(&p.param)?.ty))
            })
            .unwrap_or(Unit::None);
        args.push(format!("{name}={}", literal(value, unit)));
    }
    flag_args(node, &mut args);
    let name = names.get(node.id.as_str()).map_or("", String::as_str);
    Ok(format!("{name} = {}({})", group.name, args.join(", ")))
}

/// The statements of one scope in dependency order (document order among equals).
fn scope_lines(
    doc: &Document,
    registry: &OperatorRegistry,
    nodes: &[&Node],
    names: &BTreeMap<String, String>,
) -> Result<Vec<String>, Box<Diagnostic>> {
    let mut lines = Vec::new();
    let mut done: BTreeSet<&str> = BTreeSet::new();
    let mut pending: Vec<&Node> = nodes.to_vec();
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
            if let Some(instance) = &node.instance {
                lines.push(instance_statement(doc, registry, node, instance, names)?);
            } else {
                let spec = registry.get(&node.op).ok_or_else(|| {
                    Box::new(
                        Diagnostic::error(
                            "parse.unknown_op",
                            format!("{} is not registered", node.op),
                        )
                        .at_node(&node.id),
                    )
                })?;
                lines.push(statement(node, spec, names));
            }
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
    Ok(lines)
}

/// Groups in an order where each comes after the groups its members instance.
fn groups_in_order(doc: &Document) -> Vec<&Group> {
    fn visit<'a>(
        doc: &'a Document,
        group: &'a Group,
        seen: &mut BTreeSet<&'a GroupId>,
        out: &mut Vec<&'a Group>,
    ) {
        if !seen.insert(&group.id) {
            return;
        }
        for member in doc.members(&group.id) {
            if let Some(inner) = member.instance.as_ref().and_then(|i| doc.group(&i.group)) {
                visit(doc, inner, seen, out);
            }
        }
        out.push(group);
    }
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for group in &doc.groups {
        visit(doc, group, &mut seen, &mut out);
    }
    out
}

fn group_block(
    doc: &Document,
    registry: &OperatorRegistry,
    group: &Group,
) -> Result<Vec<String>, Box<Diagnostic>> {
    let members: Vec<&Node> = doc.members(&group.id).collect();
    let names = names(&members);
    let mut lines = vec![format!("group {}", group.name)];
    lines.extend(
        scope_lines(doc, registry, &members, &names)?
            .into_iter()
            .map(|l| format!("  {l}")),
    );
    for param in &group.params {
        let holder = names.get(param.node.as_str()).map_or("", String::as_str);
        lines.push(format!(
            "  expose {} = {holder}.{}",
            param.name, param.param
        ));
    }
    let result = names.get(group.output.as_str()).map_or("", String::as_str);
    lines.push(format!("  result {result}"));
    lines.push("end".to_owned());
    Ok(lines)
}

/// Prints the document as text: groups first, each after the groups it uses, then the top-level nodes
/// in dependency order (document order among equals).
///
/// # Errors
/// `parse.unknown_op` if a node uses an operator the registry does not know, `graph.cycle` if the
/// links loop, and `text.unsupported` for an instance that is behind its group (text always makes a
/// current instance, so upgrade it first).
pub fn to_text(registry: &OperatorRegistry, doc: &Document) -> Result<String, Box<Diagnostic>> {
    let mut lines = Vec::new();
    for group in groups_in_order(doc) {
        lines.extend(group_block(doc, registry, group)?);
    }
    let top: Vec<&Node> = doc.nodes.iter().filter(|n| n.group.is_none()).collect();
    let names = names(&top);
    lines.extend(scope_lines(doc, registry, &top, &names)?);
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
