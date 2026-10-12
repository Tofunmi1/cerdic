//! Links between nodes: connecting, disconnecting and validating types, arity and cycles.

use std::collections::BTreeMap;

use crate::apply::{Ctx, Fail, Target};
use crate::diag::Diagnostic;
use crate::doc::{Document, OUT_PORT, PortRef};
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
    let spec = reg.get(&consumer.op).ok_or_else(|| {
        Box::new(Diagnostic::error(
            "parse.unknown_op",
            format!("unknown operator {}", consumer.op),
        ))
    })?;
    let port = spec.input(&to.port).ok_or_else(|| {
        let valid: Vec<&str> = spec.inputs.iter().map(|p| p.name).collect();
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
        match arity {
            Arity::One => {
                if to.index.is_some_and(|i| i != 0) {
                    return Err(port_error(
                        "graph.port_arity",
                        "this port takes one link; leave out the index".to_owned(),
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
    }
    ctx.changed.insert(to.node.clone());
    check_graph(ctx)
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

/// The type a node produces, following pass-through operators upstream; `None` when it depends on
/// an unconnected input. The graph must be acyclic.
fn output_type(doc: &Document, reg: &OperatorRegistry, start: &NodeId) -> Option<PortType> {
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
        let Some(spec) = reg.get(&node.op) else {
            continue;
        };
        for (port, refs) in &node.inputs {
            let Some(port_spec) = spec.input(port) else {
                return Err(port_error(
                    "graph.unknown_port",
                    format!("{} has no input port {port:?}", node.op),
                    &node.id,
                    port,
                ));
            };
            let too_many = matches!(port_spec.arity, Arity::One) && refs.len() > 1;
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
