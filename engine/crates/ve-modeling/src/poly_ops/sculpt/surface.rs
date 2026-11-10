//! Geodesic brush neighbourhoods for sculpting on the mesh surface.

use std::{cmp::Ordering, collections::BinaryHeap};

use ve_kernel::ExecutionContext;

use crate::Fail;

#[derive(Clone, Copy)]
struct Frontier {
    distance: f32,
    vertex: usize,
}

impl PartialEq for Frontier {
    fn eq(&self, other: &Self) -> bool {
        self.distance.to_bits() == other.distance.to_bits() && self.vertex == other.vertex
    }
}
impl Eq for Frontier {}
impl PartialOrd for Frontier {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Frontier {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .distance
            .total_cmp(&self.distance)
            .then_with(|| other.vertex.cmp(&self.vertex))
    }
}

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

fn falloff(distance: f32, radius: f32) -> f32 {
    if distance >= radius {
        return 0.0;
    }
    let s = 1.0 - (distance / radius).powi(2);
    s * s
}

/// Returns each affected vertex and its smooth falloff weight, measured along mesh edges.
pub(super) fn reach(
    center: [f32; 3],
    radius: f32,
    positions: &[[f32; 3]],
    neighbours: &[Vec<u32>],
    context: &ExecutionContext,
) -> Result<Vec<(usize, f32)>, Fail> {
    let Some((seed, offset)) = positions
        .iter()
        .enumerate()
        .map(|(i, p)| (i, distance(*p, center)))
        .min_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)))
    else {
        return Ok(Vec::new());
    };
    if offset >= radius {
        return Ok(Vec::new());
    }
    let mut distances = vec![f32::INFINITY; positions.len()];
    let mut frontier = BinaryHeap::new();
    distances[seed] = 0.0;
    frontier.push(Frontier {
        distance: 0.0,
        vertex: seed,
    });
    let mut visited = 0_usize;
    while let Some(current) = frontier.pop() {
        if current.distance != distances[current.vertex] {
            continue;
        }
        if current.distance + offset >= radius {
            continue;
        }
        visited += 1;
        if visited.is_multiple_of(4096) && context.is_cancelled() {
            return Err(crate::fail(
                "eval.cancelled",
                "sculpt cancelled during surface falloff",
            ));
        }
        for &next in &neighbours[current.vertex] {
            let next = next as usize;
            let next_distance =
                current.distance + distance(positions[current.vertex], positions[next]);
            if next_distance + offset < radius && next_distance < distances[next] {
                distances[next] = next_distance;
                frontier.push(Frontier {
                    distance: next_distance,
                    vertex: next,
                });
            }
        }
    }
    Ok(distances
        .into_iter()
        .enumerate()
        .filter_map(|(index, surface_distance)| {
            if !surface_distance.is_finite() {
                return None;
            }
            let weight = falloff(surface_distance + offset, radius);
            (weight > 0.0).then_some((index, weight))
        })
        .collect())
}
