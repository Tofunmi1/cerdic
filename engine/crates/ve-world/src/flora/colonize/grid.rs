//! A hash grid over skeleton nodes, so each attraction point finds its nearest node without scanning all of them.

use std::collections::HashMap;

use super::super::{Skeleton, math};

/// Nodes bucketed in cubes of one cell size.
pub(super) struct NodeGrid {
    cell: f32,
    cells: HashMap<(i32, i32, i32), Vec<u32>>,
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "cells are far inside the i32 range for any plant"
)]
fn key(point: [f32; 3], cell: f32) -> (i32, i32, i32) {
    (
        (point[0] / cell).floor() as i32,
        (point[1] / cell).floor() as i32,
        (point[2] / cell).floor() as i32,
    )
}

impl NodeGrid {
    pub(super) fn new(cell: f32) -> Self {
        Self {
            cell,
            cells: HashMap::new(),
        }
    }

    pub(super) fn insert(&mut self, node: u32, position: [f32; 3]) {
        self.cells
            .entry(key(position, self.cell))
            .or_default()
            .push(node);
    }

    /// The nearest node within `radius` of `point`, with the squared distance.
    pub(super) fn nearest(
        &self,
        skeleton: &Skeleton,
        point: [f32; 3],
        radius: f32,
    ) -> Option<(u32, f32)> {
        let mut best: Option<(u32, f32)> = None;
        let limit = radius * radius;
        self.visit(point, radius, |node| {
            let delta = math::sub(point, skeleton.position[node as usize]);
            let squared = math::dot(delta, delta);
            if squared < limit && best.is_none_or(|(_, d)| squared < d) {
                best = Some((node, squared));
            }
        });
        best
    }

    /// Whether any node lies within `radius` of `point`.
    pub(super) fn any_within(&self, skeleton: &Skeleton, point: [f32; 3], radius: f32) -> bool {
        let mut found = false;
        let limit = radius * radius;
        self.visit(point, radius, |node| {
            let delta = math::sub(point, skeleton.position[node as usize]);
            found |= math::dot(delta, delta) <= limit;
        });
        found
    }

    fn visit(&self, point: [f32; 3], radius: f32, mut each: impl FnMut(u32)) {
        let (cx, cy, cz) = key(point, self.cell);
        #[expect(clippy::cast_possible_truncation, reason = "a handful of cells")]
        #[expect(clippy::cast_sign_loss, reason = "radius is positive")]
        let reach = (radius / self.cell).ceil().max(1.0) as i32;
        for dx in -reach..=reach {
            for dy in -reach..=reach {
                for dz in -reach..=reach {
                    if let Some(nodes) = self.cells.get(&(cx + dx, cy + dy, cz + dz)) {
                        for node in nodes {
                            each(*node);
                        }
                    }
                }
            }
        }
    }
}
