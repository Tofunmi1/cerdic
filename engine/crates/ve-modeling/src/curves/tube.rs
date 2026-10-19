//! `curve.tube`: a round tube along a sampled curve, with a radius that can taper (trunks, branches, vines, pipes).
//!
//! The rings come from `rings.rs` (twist-free frames, mitred at bends); this file turns them into a closed mesh and a solid.
use super::super::cache::Value;
use super::rings::{Ring, rings, segment_length};
use crate::{Fail, operations::kernel_fail};
use std::f64::consts::TAU;
use std::rc::Rc;
use ve_graph::{Node, Value as Param};
use ve_kernel::{ExecutionContext, Mesh, Solid};

/// Fewest and most stations a `radii` list may have, and the range of one radius, in metres.
const STATIONS: std::ops::RangeInclusive<usize> = 2..=16;
const RADIUS_RANGE: std::ops::RangeInclusive<f64> = 0.0001..=100.0;

fn problem(node: &Node, code: &str, message: &str, suggestion: &str) -> Fail {
    Box::new(
        ve_graph::Diagnostic::error(code, message)
            .at_node(&node.id)
            .suggest(suggestion),
    )
}

fn number(node: &Node, name: &str, default: f64) -> f64 {
    node.params
        .get(name)
        .and_then(Param::as_f64)
        .unwrap_or(default)
}

/// The radius at each station along the arc length: the `radii` list, or `radius` to `end_radius`.
fn stations(node: &Node) -> Result<Vec<f64>, Fail> {
    let list = match node.params.get("radii") {
        Some(Param::Str(text)) if !text.trim().is_empty() => text.as_str(),
        _ => {
            let start = number(node, "radius", 0.1);
            let end = number(node, "end_radius", 0.0);
            if start <= 0.0 || end < 0.0 {
                return Err(problem(
                    node,
                    "geometry.degenerate_profile",
                    "tube radius must be positive",
                    "set radius above zero",
                ));
            }
            return Ok(vec![start, if end == 0.0 { start } else { end }]);
        }
    };
    let radii: Vec<f64> = list
        .split(',')
        .map(|value| value.trim().parse::<f64>())
        .collect::<Result<_, _>>()
        .map_err(|_| {
            problem(
                node,
                "param.type",
                "radii must be comma-separated numbers",
                "use values such as radii=\"0.3, 0.2\"",
            )
        })?;
    if !STATIONS.contains(&radii.len()) || radii.iter().any(|r| !RADIUS_RANGE.contains(r)) {
        return Err(problem(
            node,
            "param.range",
            "radii needs 2 to 16 values from 0.0001 to 100",
            "provide two or more valid radii",
        ));
    }
    Ok(radii)
}

/// The radius at `fraction` (0 to 1) of the way along the arc length, linear between stations.
fn radius_at(radii: &[f64], fraction: f64) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a station count is at most 16 and the position is not negative"
    )]
    let (index, along) = {
        let position = fraction * (radii.len() - 1) as f64;
        let index = (position.floor() as usize).min(radii.len() - 2);
        (index, position - index as f64)
    };
    radii[index] + (radii[index + 1] - radii[index]) * along
}

/// The distance along the curve of each ring, from 0 to the full length (a closed curve's closing segment is last).
fn arc_lengths(node: &Node, path: &[[f64; 3]], closed: bool) -> Result<Vec<f64>, Fail> {
    let mut lengths = vec![0.0];
    let closing = closed.then(|| (path[path.len() - 1], path[0]));
    for (from, to) in path
        .windows(2)
        .map(|pair| (pair[0], pair[1]))
        .chain(closing)
    {
        let length = segment_length(from, to);
        if length <= 1.0e-12 {
            return Err(problem(
                node,
                "geometry.degenerate_curve",
                "the curve repeats a point (a segment has no length)",
                "remove the repeated points",
            ));
        }
        lengths.push(lengths.last().copied().unwrap_or(0.0) + length);
    }
    Ok(lengths)
}

/// The vertices of every ring, `sides` around each, then (when `caps`) the two cap centres.
#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "the side count is at most 64 and kernel vertices are f32"
)]
fn vertices(
    path: &[[f64; 3]],
    all: &[Ring],
    radii: &[f64],
    lengths: &[f64],
    sides: usize,
    caps: bool,
) -> Vec<[f32; 3]> {
    let total = lengths.last().copied().unwrap_or(1.0);
    let mut positions = Vec::with_capacity(path.len() * sides + if caps { 2 } else { 0 });
    for (index, point) in path.iter().enumerate() {
        let radius = radius_at(radii, lengths[index] / total);
        for side in 0..sides {
            let offset = all[index].offset(TAU * side as f64 / sides as f64, radius);
            positions.push([
                (point[0] + offset[0]) as f32,
                (point[1] + offset[1]) as f32,
                (point[2] + offset[2]) as f32,
            ]);
        }
    }
    if caps {
        positions.push(path[0].map(|c| c as f32));
        positions.push(path[path.len() - 1].map(|c| c as f32));
    }
    positions
}

/// Two triangles per quad between neighbouring rings, then (when `caps`) a fan from each cap centre.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a vertex index is below 2^32: at most 64 sides on a bounded path"
)]
fn faces(rings: usize, sides: usize, closed: bool) -> Vec<u32> {
    let vertex = |ring: usize, side: usize| (ring * sides + side % sides) as u32;
    let spans = if closed { rings } else { rings - 1 };
    let mut indices = Vec::new();
    for ring in 0..spans {
        let next_ring = (ring + 1) % rings;
        for side in 0..sides {
            let (here, across) = (vertex(ring, side), vertex(ring, side + 1));
            let (ahead, diagonal) = (vertex(next_ring, side), vertex(next_ring, side + 1));
            indices.extend_from_slice(&[here, across, ahead, across, diagonal, ahead]);
        }
    }
    if !closed {
        // The cap centres follow the ring vertices: the start's first, the end's second.
        let (start_hub, end_hub) = ((rings * sides) as u32, (rings * sides + 1) as u32);
        for side in 0..sides {
            let (start, end) = (0, rings - 1);
            indices.extend_from_slice(&[start_hub, vertex(start, side + 1), vertex(start, side)]);
            indices.extend_from_slice(&[end_hub, vertex(end, side), vertex(end, side + 1)]);
        }
    }
    indices
}

pub(super) fn build(
    node: &Node,
    inputs: &[Rc<Value>],
    context: &ExecutionContext,
) -> Result<Value, Fail> {
    let curve = inputs
        .iter()
        .find_map(|v| v.curve.as_ref())
        .ok_or_else(|| crate::fail("graph.missing_input", "tube needs a curve"))?;
    let path = &curve.points;
    if path.len() < if curve.closed { 3 } else { 2 } {
        return Err(problem(
            node,
            "geometry.unsupported_topology",
            "tube needs at least two points on an open curve or three on a closed one",
            "add more points to the curve",
        ));
    }
    // An open tube with open ends is a surface, not a solid; only a closed curve needs no caps.
    if !curve.closed && matches!(node.params.get("caps"), Some(Param::Bool(false))) {
        return Err(problem(
            node,
            "geometry.unsupported_topology",
            "a tube with open ends is not a solid",
            "leave caps on (caps=true), or use a closed curve",
        ));
    }
    let sides = match node.params.get("sides") {
        Some(Param::Int(n)) => usize::try_from((*n).clamp(3, 64)).unwrap_or(8),
        _ => 8,
    };
    let lengths = arc_lengths(node, path, curve.closed)?;
    let radii = stations(node)?;
    let all = rings(path, curve.closed).map_err(|_| {
        problem(
            node,
            "geometry.unsupported_topology",
            "the curve turns too sharply for a tube (more than 150 degrees at one point)",
            "round the curve first with curve.fillet",
        )
    })?;
    let positions = vertices(path, &all, &radii, &lengths, sides, !curve.closed);
    let indices = faces(path.len(), sides, curve.closed);
    if context.is_cancelled() {
        return Err(crate::fail("eval.cancelled", "tube cancelled"));
    }
    let solid =
        Solid::from_mesh(&Mesh { positions, indices }).map_err(|e| kernel_fail(node, &e))?;
    let mesh = solid
        .to_mesh()
        .map_err(|e| crate::fail("kernel.failed", e.to_string()))?;
    Ok(Value::solid(mesh, solid.volume()))
}
