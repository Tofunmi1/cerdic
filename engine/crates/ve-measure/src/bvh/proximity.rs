//! Smallest gap between two meshes by walking both hierarchies together, so far-apart parts cost a handful of box tests instead of a query per vertex.

use glam::Vec3;

use super::{Bvh, Node};
use crate::error::MeasureError;
use crate::geom::closest_on_triangle;
use crate::mesh::Mesh;

/// Squared distance between two nodes' boxes (zero when they overlap).
fn box_dist2(a: &Node, b: &Node) -> f32 {
    (a.min - b.max).max(b.min - a.max).max(Vec3::ZERO).length_squared()
}

impl Bvh {
    /// The smallest distance from a vertex of one mesh to the other's surface, over both directions (the same measure as testing every vertex against the other
    /// mesh's tree, in far fewer steps). Returns at once when the gap is within `stop`; each triangle pair costs 6 steps of `work`.
    ///
    /// # Errors
    /// [`MeasureError::WorkBudget`] when the steps run out.
    pub(crate) fn proximity(
        &self,
        mesh: &Mesh,
        other: &Self,
        other_mesh: &Mesh,
        stop: f32,
        (spent, budget): (&mut usize, usize),
    ) -> Result<f32, MeasureError> {
        let mut best = f32::INFINITY;
        if self.nodes.is_empty() || other.nodes.is_empty() {
            return Ok(best);
        }
        let mut stack = vec![(0u32, 0u32)];
        while let Some((na, nb)) = stack.pop() {
            let (a, b) = (self.nodes[na as usize], other.nodes[nb as usize]);
            if box_dist2(&a, &b) >= best * best {
                continue;
            }
            if a.count > 0 && b.count > 0 {
                for ka in a.start..a.start + a.count {
                    let ta = self.order[ka as usize];
                    for kb in b.start..b.start + b.count {
                        let tb = other.order[kb as usize];
                        *spent = spent.saturating_add(6);
                        if *spent > budget {
                            return Err(MeasureError::WorkBudget { limit: budget });
                        }
                        let (a0, a1, a2) = mesh.corners(ta as usize);
                        let (b0, b1, b2) = other_mesh.corners(tb as usize);
                        for p in [a0, a1, a2] {
                            best = best.min(closest_on_triangle(other_mesh, tb, p).distance);
                        }
                        for p in [b0, b1, b2] {
                            best = best.min(closest_on_triangle(mesh, ta, p).distance);
                        }
                        if best <= stop {
                            return Ok(best);
                        }
                    }
                }
                continue;
            }
            // Open the node with children (the bigger box when both have them), visiting the nearer child pair first.
            let size = |n: &Node| (n.max - n.min).length_squared();
            let open_a = b.count > 0 || (a.count == 0 && size(&a) >= size(&b));
            let pairs = if open_a {
                [(a.left, nb), (a.right, nb)]
            } else {
                [(na, b.left), (na, b.right)]
            };
            let key = |p: &(u32, u32)| box_dist2(&self.nodes[p.0 as usize], &other.nodes[p.1 as usize]);
            let (near, far) = if key(&pairs[0]) <= key(&pairs[1]) { (pairs[0], pairs[1]) } else { (pairs[1], pairs[0]) };
            stack.push(far);
            stack.push(near);
        }
        Ok(best)
    }
}
