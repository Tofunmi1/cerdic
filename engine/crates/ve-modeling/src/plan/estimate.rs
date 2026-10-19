//! Conservative operation cost estimates used before evaluation.
use crate::{Fail, fail};
use std::collections::BTreeMap;
use ve_graph::{Node, NodeId};

pub(super) fn estimate(node: &Node, sizes: &BTreeMap<NodeId, usize>) -> Result<usize, Fail> {
    match node.op.family().as_str() {
        "curve.polyline" | "curve.bezier" | "curve.circle" => {
            return Ok(crate::curves::estimate_curve(node));
        }
        family if crate::curves::is_tool(family) => {
            let input = node
                .inputs
                .values()
                .flatten()
                .map(|r| *sizes.get(&r.node).unwrap_or(&0))
                .sum();
            return Ok(crate::curves::estimate_tool(node, input));
        }
        // A drop only reads its surface: the copies are the prototype's triangles, not the surface's.
        // A bake returns its low part: only that input's triangles count, not the high part's.
        "bake.maps" => {
            return Ok(node
                .inputs
                .get("low")
                .into_iter()
                .flatten()
                .map(|r| sizes.get(&r.node).copied().unwrap_or(0))
                .sum());
        }
        "inst.drop" | "inst.scatter_on" => {
            return Ok(node
                .inputs
                .get("in")
                .into_iter()
                .flatten()
                .map(|r| sizes.get(&r.node).copied().unwrap_or(0))
                .sum());
        }
        "terrain.heightfield" => return Ok(crate::terrain::estimate(node)),
        "water.surface" => return Ok(crate::terrain::water::estimate(node)),
        "solid.sweep" => return Ok(estimate_sweep(node, sizes)),
        "curve.tube" => return Ok(estimate_tube(node, sizes)),
        "solid.loft" => return Ok(estimate_loft(node, sizes)),
        "poly.grid" => {
            return Ok(2usize
                .saturating_mul(count(node, "columns"))
                .saturating_mul(count(node, "rows")));
        }
        // Field triangle count depends on the actual zero surface, not the whole grid volume.
        // The evaluator enforces the measured kernel work cap and the final result's triangle cap.
        family if family.starts_with("field.") => return Ok(0),
        family if family.starts_with("light.") => return Ok(0),
        // A number makes no triangles.
        family if family.starts_with("value.") => return Ok(0),
        "scene.environment" | "scene.sky" => return Ok(0),
        family if family.starts_with("poly.") => return Ok(estimate_poly(node, sizes)),
        _ => {}
    }
    estimate_default(node, sizes)
}

fn estimate_default(node: &Node, sizes: &BTreeMap<NodeId, usize>) -> Result<usize, Fail> {
    let inputs = node
        .inputs
        .values()
        .flatten()
        .map(|r| *sizes.get(&r.node).unwrap_or(&0));
    let segments = bounded_count(node, "segments", 24, 3, 512);
    let divisions = bounded_count(node, "divisions", 1, 1, 256);
    match node.op.family().as_str() {
        "profile.polygon" | "profile.offset" | "profile.difference" | "profile.smooth"
        | "profile.fillet" => profile_estimate(node, inputs.sum(), segments),
        "xform.mirror" => Ok(inputs.sum::<usize>().saturating_mul(2)),
        "check.min_wall" => Ok(0),
        "profile.rect" => Ok(4),
        "profile.circle" => Ok(segments),
        "solid.torus" => Ok(segments
            .saturating_mul(count(node, "tube_segments"))
            .saturating_mul(2)),
        "xform.array" => Ok(inputs
            .sum::<usize>()
            .saturating_mul(count(node, "count_x"))
            .saturating_mul(count(node, "count_y"))
            .saturating_mul(count(node, "count_z"))),
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
        "solid.round" | "solid.chamfer" => {
            Ok(inputs.sum::<usize>().saturating_mul(12).min(250_000))
        }
        // A hull of at most `max_vertices` points has at most 2 * points - 4 triangles.
        "solid.hull" => Ok(bounded_count(node, "max_vertices", 256, 4, 2000).saturating_mul(2)),
        // An oriented box is 12 triangles; a capsule revolved from a 10-point profile is under 512.
        "solid.collision_boxes" => Ok(512),
        "solid.union" | "solid.difference" | "solid.intersect" => {
            Ok(estimate_boolean(inputs.sum()))
        }
        "xform.attach" => Ok(node
            .inputs
            .get("in")
            .into_iter()
            .flatten()
            .map(|r| sizes.get(&r.node).copied().unwrap_or(0))
            .sum()),
        "xform.move" | "xform.rotate" | "xform.scale" | "xform.align" | "frame.add"
        | "solid.material" | "solid.texture" | "solid.toon" | "solid.style" | "solid.foliage" | "anim.track" | "anim.wave" | "phys.body" | "phys.joint" | "phys.world" | "rig.bone" | "rig.skin" | "rig.ik" | "anim.parent" | "behavior.on" | "inst.array" | "inst.radial" | "inst.repeat" | "inst.mirror"
        | "inst.scatter" | "inst.jitter"
        // These deform vertex positions without changing mesh topology.
        | "xform.twist" | "xform.taper" | "xform.bend" | "xform.shear" | "xform.wave" | "xform.noise" | "xform.lattice" => Ok(inputs.sum()),
        _ => Err(fail(
            "geometry.unsupported_case",
            format!("{} has no evaluator yet", node.op),
        )),
    }
}

fn estimate_boolean(input_triangles: usize) -> usize {
    const INTERSECTION_ALLOWANCE: usize = 16_384;
    input_triangles.saturating_add(
        input_triangles
            .saturating_mul(3)
            .min(INTERSECTION_ALLOWANCE),
    )
}

pub(super) fn auxiliary_bytes(node: &Node) -> usize {
    if node.op.family().as_str() == "solid.sweep" {
        24 * 1024 * 1024
    } else {
        0
    }
}

fn estimate_sweep(node: &Node, sizes: &BTreeMap<NodeId, usize>) -> usize {
    let size = |port: &str| {
        node.inputs
            .get(port)
            .and_then(|links| links.first())
            .and_then(|link| sizes.get(&link.node))
            .copied()
            .unwrap_or(0)
    };
    let (curve, profile) = (size("curve"), size("profile"));
    curve
        .saturating_mul(profile)
        .saturating_mul(2)
        .saturating_add(profile.saturating_mul(2))
}

fn estimate_tube(node: &Node, sizes: &BTreeMap<NodeId, usize>) -> usize {
    let points = node
        .inputs
        .get("curve")
        .and_then(|links| links.first())
        .and_then(|link| sizes.get(&link.node))
        .copied()
        .unwrap_or(0);
    let side_count = bounded_count(node, "sides", 8, 3, 64);
    points
        .saturating_mul(side_count)
        .saturating_mul(2)
        .saturating_add(side_count.saturating_mul(2))
}

fn estimate_loft(node: &Node, sizes: &BTreeMap<NodeId, usize>) -> usize {
    let points = node
        .inputs
        .get("sections")
        .into_iter()
        .flatten()
        .map(|link| sizes.get(&link.node).copied().unwrap_or(0))
        .sum::<usize>();
    let section_count = node.inputs.get("sections").map_or(0, Vec::len);
    if section_count < 2 {
        return 0;
    }
    let ring = points / section_count;
    section_count
        .saturating_sub(1)
        .saturating_mul(crate::curves::loft_steps(node))
        .saturating_mul(ring)
        .saturating_mul(2)
        .saturating_add(ring.saturating_mul(2))
}

fn profile_estimate(node: &Node, inputs: usize, segments: usize) -> Result<usize, Fail> {
    match node.op.family().as_str() {
        "profile.polygon" => Ok(match node.params.get("points") {
            Some(ve_graph::Value::List(points)) => points.len(),
            _ => 0,
        }),
        "profile.offset" => {
            let round = matches!(node.params.get("join"), Some(ve_graph::Value::Str(join)) if join == "round");
            Ok(inputs.saturating_mul(2) + if round { 256 } else { 0 })
        }
        "profile.fillet" => Ok(inputs.saturating_mul(2).saturating_add(256)),
        "profile.difference" => Ok(inputs.saturating_mul(2)),
        "profile.smooth" => {
            let rounds = match node.params.get("iterations") {
                Some(ve_graph::Value::Int(value)) => (*value).clamp(0, 5),
                _ => 2,
            };
            Ok(inputs.saturating_mul(1_usize << rounds))
        }
        "profile.rect" => Ok(4),
        "profile.circle" => Ok(segments),
        _ => Err(fail(
            "geometry.unsupported_case",
            format!("{} has no profile estimate", node.op),
        )),
    }
}

/// Most triangles a face-growing polygon edit adds beyond its input: each of at most 4,096
/// individually edited faces gains four quads, and a bevel, bridge or patch extrusion is bounded by
/// the same allowance. Selections are resolved at evaluation, so the plan cannot count them; the
/// evaluation also checks the finished triangle count against the quality budget.
const POLY_GROWTH: usize = 32_768;

fn estimate_poly(node: &Node, sizes: &BTreeMap<NodeId, usize>) -> usize {
    let family = node.op.family().as_str();
    if family == "poly.select" {
        return 0;
    }
    let input: usize = node
        .inputs
        .values()
        .flatten()
        .map(|r| *sizes.get(&r.node).unwrap_or(&0))
        .sum();
    match family {
        "poly.subdivide" => {
            let levels = count(node, "levels").clamp(1, 4);
            let input_faces = input;
            input_faces
                .saturating_mul(6)
                .saturating_mul(1_usize << ((levels - 1) * 2))
        }
        // The Blender-backed edits grow a mesh by what their selection touches, which the plan cannot
        // count, so they take the same flat allowance as the other edits. (Multiplying the input here
        // would compound along a chain of edits and refuse small programs; the evaluation checks the
        // finished triangle count against the quality budget.)
        "poly.extrude_faces"
        | "poly.inset_faces"
        | "poly.bridge_loops"
        | "poly.bevel_edges"
        | "poly.bevel_network"
        | "poly.round_edges"
        | "poly.inset_region"
        | "poly.extrude_region"
        | "poly.poke"
        | "poly.subdivide_edges"
        | "poly.solidify"
        | "poly.symmetrize"
        | "poly.wireframe" => input.saturating_add(POLY_GROWTH),
        "poly.spin" => input
            .saturating_mul(count(node, "steps").clamp(2, 256))
            .min(250_000),
        "poly.mirror" => input.saturating_mul(2),
        "poly.loop_cut" => input.saturating_mul(1 + count(node, "cuts").clamp(1, 32)),
        _ => input,
    }
}

fn count(node: &Node, name: &str) -> usize {
    match node.params.get(name) {
        Some(ve_graph::Value::Int(value)) => usize::try_from(*value).unwrap_or(1),
        _ => 1,
    }
}

fn bounded_count(node: &Node, name: &str, default: usize, min: usize, max: usize) -> usize {
    match node.params.get(name) {
        Some(ve_graph::Value::Int(value)) => usize::try_from(*value)
            .ok()
            .filter(|count| (min..=max).contains(count))
            .unwrap_or(default),
        _ => default,
    }
}
