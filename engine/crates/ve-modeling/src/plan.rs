//! Preflight completes before any kernel call or geometry allocation.
use crate::{Fail, Quality, budget, fail};
use std::collections::{BTreeMap, BTreeSet};
use ve_graph::{
    Arity, Document, Node, NodeId, Output, OutputSpec, PortRef, PortType, builtin_registry,
};

pub(crate) struct Plan {
    pub(crate) order: Vec<usize>,
    pub(crate) outputs: Vec<Output>,
}

pub(crate) fn build(doc: &Document, quality: Quality) -> Result<Plan, Fail> {
    if doc.schema != ve_graph::SCHEMA {
        return Err(fail("store.newer_version", "unsupported document schema"));
    }
    if doc.units != "m" {
        return Err(fail(
            "kernel.invalid_input",
            "document coordinates must use metres",
        ));
    }
    if doc.nodes.len() > ve_graph::MAX_NODES
        || doc.canonical_json().len() > ve_graph::MAX_DOCUMENT_BYTES
    {
        return Err(fail(
            "budget.source",
            "document exceeds the node or byte limit",
        ));
    }
    let registry = builtin_registry();
    let mut done = BTreeSet::new();
    let mut types = BTreeMap::new();
    let mut sizes = BTreeMap::new();
    let mut order = Vec::new();
    let mut live_bytes = 0_usize;
    while order.len() < doc.nodes.len() {
        let before = order.len();
        for (index, node) in doc.nodes.iter().enumerate() {
            if done.contains(&node.id) {
                continue;
            }
            if node
                .inputs
                .values()
                .flatten()
                .any(|r| !done.contains(&r.node))
            {
                continue;
            }
            let spec = registry
                .get(&node.op)
                .ok_or_else(|| fail("parse.unknown_op", node.op.to_string()))?;
            validate(node, spec, &types)?;
            let kind = match spec.output {
                OutputSpec::Fixed(kind) => kind,
                OutputSpec::SameAsInput(port) => node
                    .inputs
                    .get(port)
                    .and_then(|r| r.first())
                    .and_then(|r| types.get(&r.node))
                    .copied()
                    .ok_or_else(|| fail("graph.missing_input", port))?,
            };
            let triangles = estimate(node, &sizes)?;
            if triangles > quality.triangles() {
                let issue = budget::exceeded("budget.triangles", quality.triangles(), triangles);
                return Err(Box::new((*issue).at_node(&node.id)));
            }
            live_bytes = live_bytes.saturating_add(triangles.saturating_mul(96));
            if live_bytes > 256 * 1024 * 1024 {
                return Err(budget::exceeded(
                    "budget.memory",
                    256 * 1024 * 1024,
                    live_bytes,
                ));
            }
            types.insert(node.id.clone(), kind);
            sizes.insert(node.id.clone(), triangles);
            done.insert(node.id.clone());
            order.push(index);
        }
        if before == order.len() {
            return Err(fail(
                "graph.cycle",
                "cycle, duplicate identity or missing node in graph",
            ));
        }
    }
    let outputs = outputs(doc, &types);
    validate_outputs(&outputs, &types, &sizes, quality)?;
    Ok(Plan { order, outputs })
}

fn validate_outputs(
    outputs: &[Output],
    types: &BTreeMap<NodeId, PortType>,
    sizes: &BTreeMap<NodeId, usize>,
    quality: Quality,
) -> Result<(), Fail> {
    let mut total = 0_usize;
    for output in outputs {
        if types.get(&output.from.node) != Some(&PortType::Solid) || output.from.port != "out" {
            return Err(fail(
                "graph.port_type",
                "render outputs must name a solid output",
            ));
        }
        total = total.saturating_add(*sizes.get(&output.from.node).unwrap_or(&0));
    }
    if total > quality.triangles() {
        return Err(budget::exceeded(
            "budget.triangles",
            quality.triangles(),
            total,
        ));
    }
    Ok(())
}

fn validate(
    node: &Node,
    spec: &ve_graph::OperatorSpec,
    types: &BTreeMap<NodeId, PortType>,
) -> Result<(), Fail> {
    for (name, value) in &node.params {
        let param = spec
            .param(name)
            .ok_or_else(|| fail("parse.unknown_param", name))?;
        param
            .ty
            .check(value)
            .map_err(|e| Box::new(ve_graph::Diagnostic::from_issue(e, Some(&node.id), name)))?;
    }
    for param in &spec.params {
        if !node.params.contains_key(param.name) {
            return Err(fail("parse.missing_param", param.name));
        }
    }
    for name in node.inputs.keys() {
        if spec.input(name).is_none() {
            return Err(fail("graph.unknown_port", name));
        }
    }
    for port in &spec.inputs {
        let links = node.inputs.get(port.name).map_or(&[][..], Vec::as_slice);
        let valid = match port.arity {
            Arity::One => links.len() == 1,
            Arity::Variadic { min } => (usize::from(min)..=64).contains(&links.len()),
        };
        if !valid {
            return Err(fail(
                "graph.missing_input",
                format!("{} requires {}", node.id, port.name),
            ));
        }
        for link in links {
            if link.port != "out"
                || !types
                    .get(&link.node)
                    .is_some_and(|kind| port.accepts.contains(kind))
            {
                return Err(fail(
                    "graph.port_type",
                    format!("incompatible input on {}", node.id),
                ));
            }
        }
    }
    Ok(())
}

fn estimate(node: &Node, sizes: &BTreeMap<NodeId, usize>) -> Result<usize, Fail> {
    let inputs = node
        .inputs
        .values()
        .flatten()
        .map(|r| *sizes.get(&r.node).unwrap_or(&0));
    let segments = node
        .params
        .get("segments")
        .and_then(ve_graph::Value::as_f64)
        .unwrap_or(24.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "registry bounds segments to 3..512 before estimation"
    )]
    let segments = segments as usize;
    match node.op.family().as_str() {
        "solid.box" => Ok(12),
        "solid.sphere" => Ok(segments.saturating_mul(segments).saturating_mul(2)),
        "solid.cylinder" => Ok(segments.saturating_mul(4)),
        "solid.union" | "solid.difference" | "solid.intersect" => {
            Ok(inputs.sum::<usize>().saturating_mul(4))
        }
        "xform.move" | "xform.rotate" | "xform.scale" => Ok(inputs.sum()),
        _ => Err(fail(
            "geometry.unsupported_case",
            format!("{} has no evaluator yet", node.op),
        )),
    }
}

fn outputs(doc: &Document, types: &BTreeMap<NodeId, PortType>) -> Vec<Output> {
    if !doc.outputs.is_empty() {
        return doc.outputs.clone();
    }
    let consumed: BTreeSet<_> = doc
        .nodes
        .iter()
        .flat_map(|n| n.inputs.values().flatten().map(|r| &r.node))
        .collect();
    doc.nodes
        .iter()
        .filter(|node| {
            !consumed.contains(&node.id) && types.get(&node.id) == Some(&PortType::Solid)
        })
        .map(|node| Output {
            name: node.label.clone().unwrap_or_else(|| node.id.to_string()),
            from: PortRef {
                node: node.id.clone(),
                port: "out".into(),
            },
        })
        .collect()
}
