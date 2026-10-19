//! Instance counts stay separate from prototype triangle counts.
use super::MAX_INSTANCES;
use crate::{Fail, budget, fail};
use std::collections::BTreeMap;
use ve_graph::{Node, NodeId, Value};

pub(crate) fn product(a: usize, b: usize) -> Result<usize, Fail> {
    match a.checked_mul(b) {
        Some(n) if n <= MAX_INSTANCES => Ok(n),
        _ => {
            let a = u32::try_from(a).unwrap_or(u32::MAX);
            let b = u32::try_from(b).unwrap_or(u32::MAX);
            Err(Box::new(
                ve_graph::Diagnostic::error(
                    "budget.instances",
                    "placement product exceeds the instance limit",
                )
                .with_limit(16_384.0, f64::from(a) * f64::from(b))
                .suggest("lower count_x, count_y, count_z or count"),
            ))
        }
    }
}
fn count(node: &Node, name: &str) -> usize {
    match node.params.get(name) {
        Some(Value::Int(n)) => usize::try_from(*n).unwrap_or(0),
        _ => 1,
    }
}
pub(crate) fn estimate(node: &Node, placements: &BTreeMap<NodeId, usize>) -> Result<usize, Fail> {
    let input = node
        .inputs
        .values()
        .flatten()
        .next()
        .ok_or_else(|| fail("graph.missing_input", "prototype"))?;
    let base = placements.get(&input.node).copied().unwrap_or(0).max(1);
    let copies = match node.op.family().as_str() {
        "inst.array" => product(
            product(count(node, "count_x"), count(node, "count_y"))?,
            count(node, "count_z"),
        )?,
        "inst.radial" | "inst.repeat" | "inst.scatter" => count(node, "count"),
        "inst.mirror" => 2,
        _ => 1,
    };
    product(base, copies)
}
pub(crate) fn realization(triangles: usize, count: usize) -> Result<usize, Fail> {
    let total = triangles
        .checked_mul(count)
        .ok_or_else(|| fail("budget.triangles", "realization count overflow"))?;
    if total > 4_000_000 {
        return Err(budget::exceeded("budget.triangles", 4_000_000, total));
    }
    let bytes = total
        .checked_mul(96)
        .ok_or_else(|| fail("budget.memory", "realization storage overflow"))?;
    if bytes > 256 * 1024 * 1024 {
        return Err(budget::exceeded("budget.memory", 256 * 1024 * 1024, bytes));
    }
    Ok(total)
}
