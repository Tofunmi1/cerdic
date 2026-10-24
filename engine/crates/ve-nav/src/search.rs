//! Generic deterministic A* for any [`NavSpace`](crate::NavSpace).

use crate::{CostFilter, Edge, NavError, NavSpace, NodeId};
use std::{cmp::Ordering, collections::BinaryHeap};

/// Maximum number of node expansions.
#[derive(Clone, Copy, Debug)]
pub struct SearchLimit {
    /// Expansion budget.
    pub max_expansions: usize,
}
impl Default for SearchLimit {
    fn default() -> Self {
        Self {
            max_expansions: 200_000,
        }
    }
}

/// Result of a generic node path query.
#[derive(Clone, Debug, PartialEq)]
pub struct NodePath {
    /// Nodes from start through goal.
    pub nodes: Vec<NodeId>,
    /// Sum of edge costs.
    pub cost: f32,
    /// Number of expanded nodes.
    pub expanded: usize,
}

#[derive(Clone, Copy)]
struct Entry {
    node: NodeId,
    f: f32,
    h: f32,
}
impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.node == other.node
            && self.f.to_bits() == other.f.to_bits()
            && self.h.to_bits() == other.h.to_bits()
    }
}
impl Eq for Entry {}
impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .f
            .total_cmp(&self.f)
            .then_with(|| other.h.total_cmp(&self.h))
            .then_with(|| other.node.cmp(&self.node))
    }
}

/// Finds a least-cost route using deterministic A* and a bounded expansion count.
///
/// # Errors
/// Returns `UnknownNode`, `NoRoute`, or `SearchLimit` when the query cannot complete.
pub fn find_node_path<S: NavSpace + ?Sized>(
    space: &S,
    filter: &CostFilter,
    start: NodeId,
    goal: NodeId,
    limit: SearchLimit,
) -> Result<NodePath, NavError> {
    let count = space.node_count();
    if start as usize >= count {
        return Err(NavError::UnknownNode(start));
    }
    if goal as usize >= count {
        return Err(NavError::UnknownNode(goal));
    }
    if start == goal {
        return Ok(NodePath {
            nodes: vec![start],
            cost: 0.0,
            expanded: 0,
        });
    }
    let scale = minimum_multiplier(filter);
    let heuristic = |node| {
        if scale == 0.0 {
            0.0
        } else {
            distance(space.position(node), space.position(goal)) * scale
        }
    };
    let mut costs = vec![f32::INFINITY; count];
    let mut parents = vec![NodeId::MAX; count];
    let mut closed = vec![false; count];
    let mut open = BinaryHeap::new();
    let mut edges: Vec<Edge> = Vec::new();
    costs[start as usize] = 0.0;
    let h = heuristic(start);
    open.push(Entry {
        node: start,
        f: h,
        h,
    });
    let mut expanded = 0;
    while let Some(entry) = open.pop() {
        let i = entry.node as usize;
        if closed[i] || entry.f > costs[i] + heuristic(entry.node) {
            continue;
        }
        if entry.node == goal {
            return Ok(reconstruct(start, goal, &parents, costs[i], expanded));
        }
        if expanded >= limit.max_expansions {
            return Err(NavError::SearchLimit { expanded });
        }
        closed[i] = true;
        expanded += 1;
        edges.clear();
        space.neighbours(entry.node, &mut edges);
        for edge in &edges {
            let ni = edge.to as usize;
            if ni >= count || closed[ni] {
                continue;
            }
            let Some(step) = filter.edge_cost(space.area(edge.to), edge) else {
                continue;
            };
            if !step.is_finite() || step < 0.0 {
                continue;
            }
            let candidate = costs[i] + step;
            if candidate < costs[ni] {
                costs[ni] = candidate;
                parents[ni] = entry.node;
                let h = heuristic(edge.to);
                open.push(Entry {
                    node: edge.to,
                    f: candidate + h,
                    h,
                });
            }
        }
    }
    Err(NavError::NoRoute { start, goal })
}

/// Locates two points, searches between them, and returns node positions.
///
/// # Errors
/// Returns `NoWalkableCell` if either point cannot be located, or a node-search error.
pub fn find_point_path<S: NavSpace + ?Sized>(
    space: &S,
    filter: &CostFilter,
    start: [f32; 3],
    goal: [f32; 3],
    limit: SearchLimit,
) -> Result<Vec<[f32; 3]>, NavError> {
    let a = space
        .locate(start)
        .ok_or(NavError::NoWalkableCell { at: start })?;
    let b = space
        .locate(goal)
        .ok_or(NavError::NoWalkableCell { at: goal })?;
    let path = find_node_path(space, filter, a, b, limit)?;
    Ok(path
        .nodes
        .into_iter()
        .map(|node| space.position(node))
        .collect())
}

fn minimum_multiplier(filter: &CostFilter) -> f32 {
    let mut min_area = f32::INFINITY;
    for a in 0..64 {
        if filter.allows_area(crate::AreaId(a)) {
            min_area = min_area.min(filter.area_cost(crate::AreaId(a)));
        }
    }
    let mut min_kind = f32::INFINITY;
    for k in 0..64 {
        if filter.allows_kind(crate::EdgeKind(k)) {
            min_kind = min_kind.min(filter.kind_cost(crate::EdgeKind(k)));
        }
    }
    if min_area.is_finite() && min_kind.is_finite() {
        (min_area * min_kind).min(1.0)
    } else {
        1.0
    }
}
fn distance(from: [f32; 3], to: [f32; 3]) -> f32 {
    let x = from[0] - to[0];
    let y = from[1] - to[1];
    let z = from[2] - to[2];
    (x * x + y * y + z * z).sqrt()
}
fn reconstruct(
    start: NodeId,
    goal: NodeId,
    parents: &[NodeId],
    cost: f32,
    expanded: usize,
) -> NodePath {
    let mut nodes = vec![goal];
    let mut node = goal;
    while node != start {
        node = parents[node as usize];
        nodes.push(node);
    }
    nodes.reverse();
    NodePath {
        nodes,
        cost,
        expanded,
    }
}
