//! Deterministic boundary-loop extraction.

use std::collections::{BTreeMap, BTreeSet};

use ve_kernel::ExecutionContext;

use crate::GeometryError;
use crate::topology::{EdgeInfo, check_cancelled};

pub(super) fn closed_loops(
    edges: &BTreeMap<(u32, u32), EdgeInfo>,
    context: &ExecutionContext,
) -> Result<Vec<Vec<u32>>, GeometryError> {
    let mut next = BTreeMap::new();
    let mut incoming = BTreeSet::new();
    for (index, (&(low, high), edge)) in edges.iter().enumerate() {
        if index % 1024 == 0 {
            check_cancelled(context)?;
        }
        if edge.face_count() != 1 {
            continue;
        }
        let usage = edge.uses()[0];
        let directed = if usage.forward {
            (low, high)
        } else {
            (high, low)
        };
        next.insert(directed.0, directed.1);
        incoming.insert(directed.1);
    }
    let starts: Vec<u32> = next
        .keys()
        .filter(|vertex| !incoming.contains(vertex))
        .copied()
        .collect();
    let mut consumed = BTreeSet::new();
    let mut loops = Vec::new();
    for start in starts {
        if consumed.contains(&start) {
            continue;
        }
        consume_chain(start, &next, &mut consumed, context)?;
    }
    for &start in next.keys() {
        if consumed.contains(&start) {
            continue;
        }
        let chain = consume_chain(start, &next, &mut consumed, context)?;
        if chain.len() >= 3 && chain.last() == Some(&start) {
            let mut ring = chain[..chain.len() - 1].to_vec();
            rotate_minimum(&mut ring);
            loops.push(ring);
        }
    }
    loops.sort();
    Ok(loops)
}

fn consume_chain(
    start: u32,
    next: &BTreeMap<u32, u32>,
    consumed: &mut BTreeSet<u32>,
    context: &ExecutionContext,
) -> Result<Vec<u32>, GeometryError> {
    let mut chain = vec![start];
    let mut current = start;
    loop {
        if chain.len() % 1024 == 0 {
            check_cancelled(context)?;
        }
        if !consumed.insert(current) {
            break;
        }
        let Some(&following) = next.get(&current) else {
            break;
        };
        chain.push(following);
        current = following;
        if current == start {
            break;
        }
    }
    Ok(chain)
}

fn rotate_minimum(ring: &mut [u32]) {
    if let Some((index, _)) = ring.iter().enumerate().min_by_key(|(_, vertex)| *vertex) {
        ring.rotate_left(index);
    }
}
