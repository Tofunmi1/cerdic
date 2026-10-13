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
    header(doc)?;
    plan_nodes(doc, quality)
}

fn header(doc: &Document) -> Result<(), Fail> {
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
    Ok(())
}

fn plan_nodes(doc: &Document, quality: Quality) -> Result<Plan, Fail> {
    let registry = builtin_registry();
    let mut done = BTreeSet::new();
    let mut types = BTreeMap::new();
    let mut sizes = BTreeMap::new();
    let mut placements = BTreeMap::new();
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
            let kind = output_kind(node, spec, &types)?;
            let instances = if kind == PortType::Inst {
                crate::instances::estimate(node, &placements)?
            } else {
                0
            };
            let triangles = if node.op.family().as_str() == "inst.realize" {
                let input = node
                    .inputs
                    .values()
                    .flatten()
                    .next()
                    .ok_or_else(|| fail("graph.missing_input", "instance set"))?;
                crate::instances::realization(
                    *sizes.get(&input.node).unwrap_or(&0),
                    *placements.get(&input.node).unwrap_or(&0),
                )?
            } else {
                estimate(node, &sizes)?
            };
            if triangles > quality.triangles() {
                let issue = budget::exceeded("budget.triangles", quality.triangles(), triangles);
                return Err(Box::new((*issue).at_node(&node.id)));
            }
            live_bytes = live_bytes.saturating_add(triangles.saturating_mul(96));
            live_bytes = live_bytes.saturating_add(instances.saturating_mul(128));
            if live_bytes > 256 * 1024 * 1024 {
                return Err(budget::exceeded(
                    "budget.memory",
                    256 * 1024 * 1024,
                    live_bytes,
                ));
            }
            types.insert(node.id.clone(), kind);
            sizes.insert(node.id.clone(), triangles);
            placements.insert(node.id.clone(), instances);
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
    validate_outputs(&outputs, &types, &sizes, &placements, quality)?;
    Ok(Plan { order, outputs })
}

fn output_kind(
    node: &Node,
    spec: &ve_graph::OperatorSpec,
    types: &BTreeMap<NodeId, PortType>,
) -> Result<PortType, Fail> {
    match spec.output {
        OutputSpec::Fixed(kind) => Ok(kind),
        OutputSpec::SameAsInput(port) => node
            .inputs
            .get(port)
            .and_then(|r| r.first())
            .and_then(|r| types.get(&r.node))
            .copied()
            .ok_or_else(|| fail("graph.missing_input", port)),
    }
}

fn validate_outputs(
    outputs: &[Output],
    types: &BTreeMap<NodeId, PortType>,
    sizes: &BTreeMap<NodeId, usize>,
    placements: &BTreeMap<NodeId, usize>,
    quality: Quality,
) -> Result<(), Fail> {
    let mut total = 0_usize;
    let mut instances = 0_usize;
    for output in outputs {
        if !matches!(
            types.get(&output.from.node),
            Some(PortType::Solid | PortType::Inst)
        ) || output.from.port != "out"
        {
            return Err(fail(
                "graph.port_type",
                "render outputs must name a solid output",
            ));
        }
        let count = placements
            .get(&output.from.node)
            .copied()
            .unwrap_or(0)
            .max(1);
        let drawn = sizes
            .get(&output.from.node)
            .copied()
            .unwrap_or(0)
            .checked_mul(count)
            .ok_or_else(|| fail("budget.triangles", "draw triangle product overflow"))?;
        total = total
            .checked_add(drawn)
            .ok_or_else(|| fail("budget.triangles", "result triangle sum overflow"))?;
        instances = instances
            .checked_add(*placements.get(&output.from.node).unwrap_or(&0))
            .ok_or_else(|| fail("budget.instances", "result placement sum overflow"))?;
    }
    if instances > crate::instances::MAX_INSTANCES {
        return Err(budget::exceeded(
            "budget.instances",
            crate::instances::MAX_INSTANCES,
            instances,
        ));
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
    if node.op.family().as_str() == "xform.align" {
        crate::assembly::alignment(node).map_err(|e| crate::operations::kernel_fail(node, &e))?;
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
    let divisions = node
        .params
        .get("divisions")
        .and_then(ve_graph::Value::as_f64)
        .unwrap_or(1.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "registry bounds divisions to 1..256 before estimation"
    )]
    let divisions = divisions as usize;
    // Profile nodes count points instead of triangles; they feed the solids built from them.
    match node.op.family().as_str() {
        "profile.polygon" => Ok(match node.params.get("points") {
            Some(ve_graph::Value::List(points)) => points.len(),
            _ => 0,
        }),
        "profile.offset" => {
            // The arcs of a round join turn through 2 pi per loop in total, so they add at most 64
            // points per loop (64 segments per circle); a few loops are covered by the constant.
            let round =
                matches!(node.params.get("join"), Some(ve_graph::Value::Str(j)) if j == "round");
            Ok(inputs.sum::<usize>().saturating_mul(2) + if round { 256 } else { 0 })
        }
        // A mirror doubles its input, like a profile difference doubles its inputs.
        "profile.difference" | "xform.mirror" => Ok(inputs.sum::<usize>().saturating_mul(2)),
        "profile.smooth" => {
            let rounds = match node.params.get("iterations") {
                Some(ve_graph::Value::Int(n)) => (*n).clamp(0, 5),
                _ => 2,
            };
            Ok(inputs.sum::<usize>().saturating_mul(1_usize << rounds))
        }
        "check.min_wall" => Ok(0),
        "profile.rect" => Ok(4),
        "profile.circle" => Ok(segments),
        "solid.torus" => Ok(segments
            .saturating_mul(count(node, "tube_segments"))
            .saturating_mul(2)),
        // Copies are counted before any is built, so a pattern over the budget is refused cheaply.
        "xform.array" => Ok(inputs.sum::<usize>().saturating_mul(
            count(node, "count_x")
                .saturating_mul(count(node, "count_y"))
                .saturating_mul(count(node, "count_z")),
        )),
        "xform.radial" | "xform.repeat" => {
            Ok(inputs.sum::<usize>().saturating_mul(count(node, "count")))
        }
        "solid.extrude" => Ok(inputs
            .sum::<usize>()
            .saturating_mul(divisions.saturating_mul(4).saturating_add(2))),
        "solid.revolve" => Ok(inputs
            .sum::<usize>()
            .saturating_mul(segments)
            .saturating_mul(2)),
        "solid.box" => Ok(12),
        "solid.sphere" | "solid.capsule" => Ok(segments.saturating_mul(segments).saturating_mul(2)),
        "solid.cylinder" | "solid.cone" => Ok(segments.saturating_mul(4)),
        "solid.union" | "solid.difference" | "solid.intersect" => {
            Ok(inputs.sum::<usize>().saturating_mul(4))
        }
        "xform.move" | "xform.rotate" | "xform.scale" | "xform.align" | "solid.material"
        | "inst.array" | "inst.radial" | "inst.repeat" | "inst.mirror" => Ok(inputs.sum()),
        _ => Err(fail(
            "geometry.unsupported_case",
            format!("{} has no evaluator yet", node.op),
        )),
    }
}

/// A whole-number parameter as a size; the registry has already bounded it, a missing one counts as 1.
fn count(node: &Node, name: &str) -> usize {
    match node.params.get(name) {
        Some(ve_graph::Value::Int(n)) => usize::try_from(*n).unwrap_or(1),
        _ => 1,
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
            !consumed.contains(&node.id)
                && matches!(types.get(&node.id), Some(PortType::Solid | PortType::Inst))
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
