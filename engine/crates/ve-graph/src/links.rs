//! Links between nodes: connecting, disconnecting and validating types, arity and cycles.

use std::collections::BTreeMap;

use crate::apply::{Ctx, Fail, Target};
use crate::diag::Diagnostic;
use crate::doc::{Document, Node, OUT_PORT, PortRef};
use crate::id::NodeId;
use crate::limits::{MAX_VARIADIC, count};
use crate::nodes::unknown_node;
use crate::spec::{Arity, OperatorRegistry, OutputSpec, PortType};

fn port_error(code: &str, message: String, node: &NodeId, port: &str) -> Fail {
    Box::new(Diagnostic::error(code, message).at_node(node).at_port(port))
}

pub(crate) fn connect(ctx: &mut Ctx<'_>, from: &PortRef, to: &Target) -> Result<(), Fail> {
    if from.port != OUT_PORT {
        return Err(port_error(
            "graph.unknown_port",
            format!("{} has no output port {:?}", from.node, from.port),
            &from.node,
            &from.port,
        ));
    }
    ctx.doc
        .node(&from.node)
        .ok_or_else(|| unknown_node(&from.node))?;
    let consumer = ctx
        .doc
        .node(&to.node)
        .ok_or_else(|| unknown_node(&to.node))?;
    let reg = ctx.reg;
    reg.get(&consumer.op).ok_or_else(|| {
        Box::new(Diagnostic::error(
            "parse.unknown_op",
            format!("unknown operator {}", consumer.op),
        ))
    })?;
    let port = port_rule(&ctx.doc, reg, consumer, &to.port).ok_or_else(|| {
        let valid = port_names(&ctx.doc, reg, consumer);
        Box::new(
            Diagnostic::error(
                "graph.unknown_port",
                format!("{} has no input port {:?}", consumer.op, to.port),
            )
            .at_node(&to.node)
            .at_port(&to.port)
            .suggest(format!(
                "input ports: {}",
                if valid.is_empty() {
                    "none".to_owned()
                } else {
                    valid.join(", ")
                }
            )),
        )
    })?;
    let arity = port.arity;
    if let Some(node) = ctx.doc.node_mut(&to.node) {
        let links = node.inputs.entry(to.port.clone()).or_default();
        write_link(links, arity, from, to)?;
    }
    ctx.changed.insert(to.node.clone());
    check_graph(ctx)
}

fn write_link(
    links: &mut Vec<PortRef>,
    arity: Arity,
    from: &PortRef,
    to: &Target,
) -> Result<(), Fail> {
    match arity {
        Arity::One | Arity::Optional => {
            let hint = if arity == Arity::One {
                "this port takes one link; leave out the index"
            } else {
                "this port takes at most one link; leave out the index"
            };
            if to.index.is_some_and(|index| index != 0) {
                return Err(port_error(
                    "graph.port_arity",
                    hint.to_owned(),
                    &to.node,
                    &to.port,
                ));
            }
            *links = vec![from.clone()];
        }
        Arity::Variadic { .. } => {
            let at = to.index.unwrap_or(links.len());
            if at > links.len() || links.len() >= MAX_VARIADIC {
                return Err(Box::new(
                    Diagnostic::error("graph.port_arity", "no room for that link")
                        .at_node(&to.node)
                        .at_port(&to.port)
                        .with_limit(count(MAX_VARIADIC), count(links.len() + 1)),
                ));
            }
            links.insert(at, from.clone());
        }
    }
    Ok(())
}

pub(crate) fn disconnect(ctx: &mut Ctx<'_>, to: &Target) -> Result<(), Fail> {
    ctx.doc
        .node(&to.node)
        .ok_or_else(|| unknown_node(&to.node))?;
    let Some(node) = ctx.doc.node_mut(&to.node) else {
        return Ok(());
    };
    let links = node.inputs.get_mut(&to.port);
    let Some(links) = links.filter(|l| !l.is_empty()) else {
        return Err(port_error(
            "graph.port_arity",
            format!("{:?} has no link to remove", to.port),
            &to.node,
            &to.port,
        ));
    };
    let at = match to.index {
        Some(i) if i < links.len() => i,
        None if links.len() == 1 => 0,
        _ => {
            return Err(port_error(
                "graph.port_arity",
                "give the index of the link to remove".to_owned(),
                &to.node,
                &to.port,
            ));
        }
    };
    links.remove(at);
    if links.is_empty() {
        node.inputs.remove(&to.port);
    }
    ctx.changed.insert(to.node.clone());
    Ok(())
}

/// What one input port accepts and how many links it takes.
pub(crate) struct PortRule {
    pub(crate) accepts: &'static [PortType],
    pub(crate) arity: Arity,
}

const SOLID_ONLY: &[PortType] = &[PortType::Solid];

/// The rule for the named input of `node`. An instance's inputs are its group's `group.input`
/// members, bound by name; each takes one solid.
pub(crate) fn port_rule(
    doc: &Document,
    reg: &OperatorRegistry,
    node: &Node,
    port: &str,
) -> Option<PortRule> {
    if let Some(instance) = &node.instance {
        return doc
            .group_inputs(&instance.group)
            .iter()
            .any(|input| input.name == port)
            .then_some(PortRule {
                accepts: SOLID_ONLY,
                arity: Arity::One,
            });
    }
    reg.get(&node.op)?.input(port).map(|spec| PortRule {
        accepts: spec.accepts,
        arity: spec.arity,
    })
}

/// The input port names of `node`, for suggestions.
fn port_names(doc: &Document, reg: &OperatorRegistry, node: &Node) -> Vec<String> {
    if let Some(instance) = &node.instance {
        return doc
            .group_inputs(&instance.group)
            .into_iter()
            .map(|input| input.name)
            .collect();
    }
    reg.get(&node.op)
        .map(|spec| spec.inputs.iter().map(|p| p.name.to_owned()).collect())
        .unwrap_or_default()
}

/// The type a node produces, following pass-through operators upstream; `None` when it depends on
/// an unconnected input. The graph must be acyclic.
pub(crate) fn output_type(
    doc: &Document,
    reg: &OperatorRegistry,
    start: &NodeId,
) -> Option<PortType> {
    let mut current = doc.node(start)?;
    for _ in 0..=doc.nodes.len() {
        match reg.get(&current.op)?.output {
            OutputSpec::Fixed(t) => return Some(t),
            OutputSpec::SameAsInput(port) => {
                let source = current.inputs.get(port)?.first()?;
                current = doc.node(&source.node)?;
            }
        }
    }
    None
}

/// Finds a cycle among the links, returning the nodes on it.
fn find_cycle(doc: &Document) -> Option<Vec<NodeId>> {
    let index: BTreeMap<&NodeId, usize> = doc
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (&n.id, i))
        .collect();
    let children = |i: usize| -> Vec<usize> {
        doc.nodes
            .get(i)
            .map(|n| {
                n.inputs
                    .values()
                    .flatten()
                    .filter_map(|r| index.get(&r.node).copied())
                    .collect()
            })
            .unwrap_or_default()
    };
    // 0 = unvisited, 1 = on the current path, 2 = finished.
    let mut state = vec![0_u8; doc.nodes.len()];
    for start in 0..doc.nodes.len() {
        if state.get(start).copied() != Some(0) {
            continue;
        }
        let mut path: Vec<(usize, Vec<usize>)> = vec![(start, children(start))];
        if let Some(s) = state.get_mut(start) {
            *s = 1;
        }
        while let Some((node, pending)) = path.last_mut() {
            let node = *node;
            let Some(next) = pending.pop() else {
                if let Some(s) = state.get_mut(node) {
                    *s = 2;
                }
                path.pop();
                continue;
            };
            match state.get(next).copied() {
                Some(1) => {
                    let from = path.iter().position(|(n, _)| *n == next).unwrap_or(0);
                    return Some(
                        path.iter()
                            .skip(from)
                            .filter_map(|(n, _)| doc.nodes.get(*n).map(|n| n.id.clone()))
                            .collect(),
                    );
                }
                Some(0) => {
                    if let Some(s) = state.get_mut(next) {
                        *s = 1;
                    }
                    path.push((next, children(next)));
                }
                _ => {}
            }
        }
    }
    None
}

/// Checks every link: the ports exist, the source exists, arity holds, types fit, no cycles.
pub(crate) fn check_graph(ctx: &Ctx<'_>) -> Result<(), Fail> {
    let (doc, reg) = (&ctx.doc, ctx.reg);
    if let Some(cycle) = find_cycle(doc) {
        let first = cycle.first().cloned();
        let mut d = Diagnostic::error("graph.cycle", "these nodes would feed each other")
            .with_related(cycle);
        d.node = first;
        return Err(Box::new(d));
    }
    for node in &doc.nodes {
        if reg.get(&node.op).is_none() {
            continue;
        }
        // An instance that is behind its group may still link to inputs the group has since lost; it
        // stays editable so it can be upgraded.
        if node.instance.as_ref().is_some_and(|i| {
            doc.group(&i.group)
                .is_none_or(|group| group.version != i.version)
        }) {
            continue;
        }
        for (port, refs) in &node.inputs {
            let Some(port_spec) = port_rule(doc, reg, node, port) else {
                return Err(port_error(
                    "graph.unknown_port",
                    format!("{} has no input port {port:?}", node.op),
                    &node.id,
                    port,
                ));
            };
            let too_many =
                matches!(port_spec.arity, Arity::One | Arity::Optional) && refs.len() > 1;
            if too_many || refs.len() > MAX_VARIADIC {
                return Err(port_error(
                    "graph.port_arity",
                    format!("too many links on {port:?}"),
                    &node.id,
                    port,
                ));
            }
            for source in refs {
                if doc.node(&source.node).is_none() {
                    return Err(unknown_node(&source.node));
                }
                if let Some(t) = output_type(doc, reg, &source.node)
                    && !port_spec.accepts.contains(&t)
                {
                    let accepted: Vec<String> = port_spec
                        .accepts
                        .iter()
                        .map(|a| format!("{a:?}").to_lowercase())
                        .collect();
                    return Err(Box::new(
                        Diagnostic::error(
                            "graph.port_type",
                            format!(
                                "a {} cannot go into {port:?}, which takes {}",
                                format!("{t:?}").to_lowercase(),
                                accepted.join(" or ")
                            ),
                        )
                        .at_node(&node.id)
                        .at_port(port)
                        .with_related(vec![source.node.clone()]),
                    ));
                }
            }
        }
    }
    Ok(())
}
