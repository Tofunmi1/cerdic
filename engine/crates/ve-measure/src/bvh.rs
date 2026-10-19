//! A deterministic bounding-volume hierarchy for ray casts and closest-point queries.

use glam::Vec3;

use crate::error::MeasureError;
use crate::geom::{ClosestPoint, RayHit, aabb_dist2, closest_on_triangle, hit_triangle, slab};
use crate::mesh::Mesh;

/// How many triangles a leaf holds before the hierarchy splits.
const LEAF_SIZE: usize = 4;

/// A sampled symmetric Hausdorff distance between two meshes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hausdorff {
    /// The largest sample-to-surface distance, in metres.
    pub max: f32,
    /// The mean sample-to-surface distance, in metres.
    pub mean: f32,
}

#[derive(Clone, Copy, Debug)]
struct Node {
    min: Vec3,
    max: Vec3,
    /// Leaf: first index into `order`. Internal: unused.
    start: u32,
    /// Leaf: how many triangles. Internal: zero.
    count: u32,
    /// Internal: the two child node indices. Leaf: equal to the node's own index.
    left: u32,
    right: u32,
}

/// A bounded bounding-volume hierarchy over a mesh's triangles.
///
/// The split rule is deterministic: each node splits its triangle list at the median of the
/// longest centroid axis, ties broken by triangle index, so the same mesh always gives the same
/// tree.
#[derive(Clone, Debug)]
pub struct Bvh {
    nodes: Vec<Node>,
    order: Vec<u32>,
}

impl Bvh {
    /// Build the hierarchy. Fails only on malformed input.
    ///
    /// # Errors
    /// Returns [`MeasureError`] when the mesh is malformed (empty, bad indices, non-finite
    /// coordinates or mismatched uvs).
    pub fn build(mesh: &Mesh) -> Result<Self, MeasureError> {
        let triangles = mesh.validate()?;
        let mut order: Vec<u32> = (0..triangles as u32).collect();
        let mut bvh = Self {
            nodes: Vec::new(),
            order: Vec::new(),
        };
        bvh.build_node(mesh, &mut order);
        Ok(bvh)
    }

    /// How many triangles the hierarchy covers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.order.len()
    }

    /// True when the hierarchy covers no triangles.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// Cast a ray and return the nearest hit, or `None`.
    #[must_use]
    pub fn raycast(&self, mesh: &Mesh, origin: [f32; 3], dir: [f32; 3]) -> Option<RayHit> {
        if self.nodes.is_empty() {
            return None;
        }
        let origin = Vec3::from_array(origin);
        let dir = Vec3::from_array(dir);
        let inv = Vec3::new(1.0 / dir.x, 1.0 / dir.y, 1.0 / dir.z);
        let mut best: Option<RayHit> = None;
        let mut stack = vec![0u32];
        while let Some(n) = stack.pop() {
            let node = self.nodes[n as usize];
            if !slab(
                origin,
                inv,
                node.min,
                node.max,
                best.map_or(f32::INFINITY, |h| h.distance),
            ) {
                continue;
            }
            if node.count > 0 {
                for k in node.start..node.start + node.count {
                    let tri = self.order[k as usize];
                    if let Some(hit) = hit_triangle(mesh, tri, origin, dir)
                        && best.is_none_or(|b| hit.distance < b.distance)
                    {
                        best = Some(hit);
                    }
                }
            } else {
                stack.push(node.left);
                stack.push(node.right);
            }
        }
        best
    }

    /// The point on the mesh nearest to `point`, or `None` for an empty hierarchy.
    #[must_use]
    pub fn closest_point(&self, mesh: &Mesh, point: [f32; 3]) -> Option<ClosestPoint> {
        if self.nodes.is_empty() {
            return None;
        }
        let p = Vec3::from_array(point);
        let mut best: Option<ClosestPoint> = None;
        let mut stack = vec![0u32];
        while let Some(n) = stack.pop() {
            let node = self.nodes[n as usize];
            let bound = aabb_dist2(p, node.min, node.max);
            if best.is_some_and(|b| bound >= b.distance * b.distance) {
                continue;
            }
            if node.count > 0 {
                for k in node.start..node.start + node.count {
                    let tri = self.order[k as usize];
                    let cp = closest_on_triangle(mesh, tri, p);
                    if best.is_none_or(|b| cp.distance < b.distance) {
                        best = Some(cp);
                    }
                }
            } else {
                stack.push(node.left);
                stack.push(node.right);
            }
        }
        best
    }

    fn build_node(&mut self, mesh: &Mesh, tris: &mut [u32]) -> u32 {
        let (min, max) = bounds_of(mesh, tris);
        let index = self.nodes.len() as u32;
        self.nodes.push(Node {
            min,
            max,
            start: 0,
            count: 0,
            left: index,
            right: index,
        });
        if tris.len() <= LEAF_SIZE {
            let start = self.order.len() as u32;
            self.order.extend_from_slice(tris);
            self.nodes[index as usize] = Node {
                min,
                max,
                start,
                count: tris.len() as u32,
                left: index,
                right: index,
            };
            return index;
        }
        let axis = longest_centroid_axis(mesh, tris);
        tris.sort_unstable_by(|&a, &b| centroid_cmp(mesh, a, b, axis));
        let mid = tris.len() / 2;
        let (left_tris, right_tris) = tris.split_at_mut(mid);
        let left = self.build_node(mesh, left_tris);
        let right = self.build_node(mesh, right_tris);
        self.nodes[index as usize] = Node {
            min,
            max,
            start: 0,
            count: 0,
            left,
            right,
        };
        index
    }
}

/// Cast a ray at a mesh, building a hierarchy on the fly.
///
/// # Errors
/// Returns [`MeasureError`] when the mesh is malformed.
pub fn raycast(
    mesh: &Mesh,
    origin: [f32; 3],
    dir: [f32; 3],
) -> Result<Option<RayHit>, MeasureError> {
    Ok(Bvh::build(mesh)?.raycast(mesh, origin, dir))
}

/// The nearest point on a mesh to `point`, building a hierarchy on the fly.
///
/// # Errors
/// Returns [`MeasureError`] when the mesh is malformed or empty.
pub fn closest_point(mesh: &Mesh, point: [f32; 3]) -> Result<ClosestPoint, MeasureError> {
    Bvh::build(mesh)?
        .closest_point(mesh, point)
        .ok_or(MeasureError::Empty)
}

/// The sampled symmetric Hausdorff distance between two meshes.
///
/// Samples are the vertices and triangle centroids of each mesh; for every sample the distance to
/// the other surface is measured, and `max` and `mean` are taken over both directions. Comparing a
/// high-poly mesh with a low-poly one therefore costs `O((V + F))` closest-point queries.
///
/// # Errors
/// Returns [`MeasureError`] when either mesh is malformed.
pub fn distance_between(a: &Mesh, b: &Mesh) -> Result<Hausdorff, MeasureError> {
    let bvh_a = Bvh::build(a)?;
    let bvh_b = Bvh::build(b)?;
    let mut max = 0.0f32;
    let mut sum = 0.0f32;
    let mut count = 0usize;
    for p in samples(a) {
        let cp = bvh_b.closest_point(b, p).ok_or(MeasureError::Empty)?;
        max = max.max(cp.distance);
        sum += cp.distance;
        count += 1;
    }
    for p in samples(b) {
        let cp = bvh_a.closest_point(a, p).ok_or(MeasureError::Empty)?;
        max = max.max(cp.distance);
        sum += cp.distance;
        count += 1;
    }
    let mean = if count > 0 { sum / count as f32 } else { 0.0 };
    Ok(Hausdorff { max, mean })
}

fn samples(mesh: &Mesh) -> Vec<[f32; 3]> {
    let mut out: Vec<[f32; 3]> = mesh.positions.to_vec();
    for t in 0..mesh.indices.len() / 3 {
        let (a, b, c) = mesh.corners(t);
        out.push(((a + b + c) / 3.0).to_array());
    }
    out
}

fn centroid(mesh: &Mesh, t: u32) -> Vec3 {
    let (a, b, c) = mesh.corners(t as usize);
    (a + b + c) / 3.0
}

fn bounds_of(mesh: &Mesh, tris: &[u32]) -> (Vec3, Vec3) {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for &t in tris {
        let (a, b, c) = mesh.corners(t as usize);
        for p in [a, b, c] {
            min = min.min(p);
            max = max.max(p);
        }
    }
    (min, max)
}

fn longest_centroid_axis(mesh: &Mesh, tris: &[u32]) -> usize {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for &t in tris {
        let c = centroid(mesh, t);
        min = min.min(c);
        max = max.max(c);
    }
    let size = max - min;
    if size.x >= size.y && size.x >= size.z {
        0
    } else if size.y >= size.z {
        1
    } else {
        2
    }
}

fn centroid_cmp(mesh: &Mesh, a: u32, b: u32, axis: usize) -> std::cmp::Ordering {
    let ca = centroid(mesh, a).to_array()[axis];
    let cb = centroid(mesh, b).to_array()[axis];
    ca.partial_cmp(&cb)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| a.cmp(&b))
}
