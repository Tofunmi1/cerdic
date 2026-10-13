//! Metadata growth is predicted before geometry work or copy allocation.
use super::MAX_FRAMES;
use crate::{Fail, budget};
use std::collections::BTreeMap;
use ve_graph::{Node, NodeId, Value};

pub(crate) fn estimate(
    node: &Node,
    counts: &BTreeMap<NodeId, usize>,
    placements: &BTreeMap<NodeId, usize>,
) -> Result<usize, Fail> {
    let input = node.inputs.get("in").and_then(|links| links.first());
    let base = input
        .and_then(|link| counts.get(&link.node))
        .copied()
        .unwrap_or(0);
    if node.flags.muted {
        return Ok(0);
    }
    if node.flags.bypass {
        return Ok(base);
    }
    let count = |name| match node.params.get(name) {
        Some(Value::Int(n)) => usize::try_from(*n).unwrap_or(0),
        _ => 1,
    };
    let copies = match node.op.family().as_str() {
        "xform.mirror" => 2,
        "xform.array" => count("count_x")
            .saturating_mul(count("count_y"))
            .saturating_mul(count("count_z")),
        "xform.radial" | "xform.repeat" => count("count"),
        "inst.realize" => input
            .and_then(|link| placements.get(&link.node))
            .copied()
            .unwrap_or(0),
        _ => 1,
    };
    let observed = base
        .checked_mul(copies)
        .and_then(|n| n.checked_add(usize::from(node.op.family().as_str() == "frame.add")));
    let observed =
        observed.ok_or_else(|| crate::fail("budget.sockets", "socket count overflow"))?;
    if observed > MAX_FRAMES {
        let error = budget::exceeded("budget.sockets", MAX_FRAMES, observed);
        return Err(Box::new((*error).at_node(&node.id)));
    }
    Ok(observed)
}
