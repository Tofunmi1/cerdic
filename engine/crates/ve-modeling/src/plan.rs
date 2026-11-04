//! Preflight completes before any kernel call or geometry allocation.
mod estimate;
mod fields;
mod validate;

use crate::{Fail, Quality, budget, fail, fail_suggest};
use estimate::estimate;
use std::collections::{BTreeMap, BTreeSet};
use validate::validate;
use ve_graph::{Document, Node, NodeId, Output, OutputSpec, PortRef, PortType, builtin_registry};

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
    let mut field_complexities = BTreeMap::new();
    let mut placements = BTreeMap::new();
    // Nodes whose copies are drawn only inside a camera-distance band (`scatter.place` show_from / show_to, and whatever is built from it).
    let mut banded = BTreeSet::new();
    let mut frames = BTreeMap::new();
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
            let sockets = crate::frames::estimate(node, &frames, &placements)?;
            let kind = output_kind(node, spec, &types)?;
            let instances = if kind == PortType::Inst {
                crate::instances::estimate(node, &placements)?
            } else {
                0
            };
            let triangles =
                estimated_triangles(node, quality, &sizes, &placements, &field_complexities)?;
            if triangles > quality.triangles() {
                let issue = budget::exceeded("budget.triangles", quality.triangles(), triangles);
                return Err(Box::new((*issue).at_node(&node.id)));
            }
            live_bytes = live_bytes.saturating_add(triangles.saturating_mul(96));
            live_bytes = live_bytes.saturating_add(estimate::auxiliary_bytes(node));
            live_bytes = live_bytes.saturating_add(instances.saturating_mul(128));
            live_bytes = live_bytes.saturating_add(sockets.saturating_mul(256));
            if live_bytes > budget::MEMORY_LIMIT {
                return Err(budget::exceeded(
                    "budget.memory",
                    budget::MEMORY_LIMIT,
                    live_bytes,
                ));
            }
            types.insert(node.id.clone(), kind);
            sizes.insert(node.id.clone(), triangles);
            if let Some(complexity) = fields::complexity(node, &field_complexities) {
                field_complexities.insert(node.id.clone(), complexity);
            }
            placements.insert(node.id.clone(), instances);
            let own_band = node.op.family().as_str() == "scatter.place"
                && ["show_from", "show_to"].iter().any(|name| {
                    node.params
                        .get(*name)
                        .and_then(ve_graph::Value::as_f64)
                        .is_some_and(|v| v > 0.0)
                });
            if own_band
                || node
                    .inputs
                    .values()
                    .flatten()
                    .any(|r| banded.contains(&r.node))
            {
                banded.insert(node.id.clone());
            }
            frames.insert(node.id.clone(), sockets);
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
    validate_outputs(doc, &outputs, &types, &sizes, &placements, &banded, quality)?;
    Ok(Plan { order, outputs })
}

fn estimated_triangles(
    node: &Node,
    quality: Quality,
    sizes: &BTreeMap<NodeId, usize>,
    placements: &BTreeMap<NodeId, usize>,
    fields: &BTreeMap<NodeId, fields::Complexity>,
) -> Result<usize, Fail> {
    match node.op.family().as_str() {
        "field.to_solid" => {
            let complexity = node
                .inputs
                .get("field")
                .and_then(|links| links.first())
                .and_then(|link| fields.get(&link.node))
                .copied()
                .ok_or_else(|| fail("graph.missing_input", "field.to_solid source cost"))?;
            fields::mesh_estimate(node, complexity, quality)
                .map_err(|issue| Box::new((*issue).at_node(&node.id)))
        }
        "inst.realize" => {
            let input = node
                .inputs
                .values()
                .flatten()
                .next()
                .ok_or_else(|| fail("graph.missing_input", "instance set"))?;
            crate::instances::realization(
                *sizes.get(&input.node).unwrap_or(&0),
                *placements.get(&input.node).unwrap_or(&0),
            )
        }
        _ => estimate(node, sizes),
    }
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
            .ok_or_else(|| {
                fail_suggest(
                    "graph.missing_input",
                    port,
                    format!("link the {port} input"),
                )
            }),
    }
}

fn validate_outputs(
    doc: &Document,
    outputs: &[Output],
    types: &BTreeMap<NodeId, PortType>,
    sizes: &BTreeMap<NodeId, usize>,
    placements: &BTreeMap<NodeId, usize>,
    banded: &BTreeSet<NodeId>,
    quality: Quality,
) -> Result<(), Fail> {
    // Stored triangles count a placed shape once; drawn ones count every placement (see `budget::Triangles`).
    let mut total = 0_usize;
    let mut total_drawn = 0_usize;
    let mut instances = 0_usize;
    let mut lights = 0_usize;
    for output in outputs {
        let output_kind = types.get(&output.from.node);
        let game_output = output_kind == Some(&PortType::Game)
            && output.name == "rules"
            && doc
                .node(&output.from.node)
                .is_some_and(|node| node.op.family().as_str() == "game.collect");
        if !matches!(
            output_kind,
            Some(
                PortType::Solid
                    | PortType::Inst
                    | PortType::Poly
                    | PortType::Light
                    | PortType::Environment
            )
        ) && !game_output
            || output.from.port != "out"
        {
            if output_kind == Some(&PortType::Pattern) {
                return Err(fail_suggest(
                    "graph.port_type",
                    format!(
                        "{} is a pattern (a function of position), not geometry, so it cannot be a scene output",
                        output.from.node
                    ),
                    "link it into a node that takes a pattern, such as the `where` input of scatter.place, and output that node",
                ));
            }
            if output_kind == Some(&PortType::Volume) {
                return Err(fail_suggest(
                    "graph.port_type",
                    format!(
                        "{} is a volume (a grid of distances), not geometry, so it cannot be a scene output",
                        output.from.node
                    ),
                    "turn it into a mesh with volume.to_mesh and output that",
                ));
            }
            if output_kind == Some(&PortType::Game) {
                return Err(fail_suggest(
                    "graph.port_type",
                    format!(
                        "{} is a game definition, not geometry, so it cannot be a scene output",
                        output.from.node
                    ),
                    "output a solid, or use this node as the `rules` output",
                ));
            }
            return Err(fail_suggest(
                "graph.port_type",
                "scene outputs must name geometry, a light or the environment",
                "output a solid, an instance, a polygon, a light or the environment",
            ));
        }
        if output_kind == Some(&PortType::Light) {
            lights += 1;
            if lights > crate::MAX_SCENE_LIGHTS {
                return Err(budget::exceeded(
                    "budget.lights",
                    crate::MAX_SCENE_LIGHTS,
                    lights,
                ));
            }
            continue;
        }
        if output_kind == Some(&PortType::Environment) {
            continue;
        }
        // A banded set shows only the copies one camera reaches, which depends on where they stand; the check after evaluation
        // (`budget::drawn_copies`) counts that exactly, so here it is counted once.
        let count = if banded.contains(&output.from.node) {
            1
        } else {
            placements
                .get(&output.from.node)
                .copied()
                .unwrap_or(0)
                .max(1)
        };
        let stored = sizes.get(&output.from.node).copied().unwrap_or(0);
        let drawn = stored
            .checked_mul(count)
            .ok_or_else(|| fail("budget.drawn_triangles", "draw triangle product overflow"))?;
        total = total
            .checked_add(stored)
            .ok_or_else(|| fail("budget.triangles", "result triangle sum overflow"))?;
        total_drawn = total_drawn
            .checked_add(drawn)
            .ok_or_else(|| fail("budget.drawn_triangles", "result triangle sum overflow"))?;
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
    if total_drawn > quality.drawn_triangles() {
        return Err(budget::exceeded(
            "budget.drawn_triangles",
            quality.drawn_triangles(),
            total_drawn,
        ));
    }
    Ok(())
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
                && matches!(
                    types.get(&node.id),
                    Some(
                        PortType::Solid
                            | PortType::Inst
                            | PortType::Poly
                            | PortType::Light
                            | PortType::Environment
                    )
                )
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
