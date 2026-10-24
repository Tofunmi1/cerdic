//! Dynamic node blockers and destination cost volumes.
use crate::{AreaId, Edge, NavSpace, NodeId, NodePath};

/// Identifier for a dynamic obstacle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObstacleId(pub u32);
/// Identifier for a destination cost volume.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VolumeId(pub u32);
#[derive(Clone, Copy)]
enum Shape {
    Box([f32; 3], [f32; 3]),
    Circle([f32; 3], f32),
}
#[derive(Clone, Copy)]
struct Volume {
    min: [f32; 3],
    max: [f32; 3],
    multiplier: f32,
}
/// A navigation space with runtime blockers and destination costs.
pub struct DynamicSpace<S: NavSpace> {
    inner: S,
    obstacles: Vec<Option<Shape>>,
    volumes: Vec<Option<Volume>>,
    version: u64,
}
impl<S: NavSpace> DynamicSpace<S> {
    /// Wraps a navigation space.
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            obstacles: Vec::new(),
            volumes: Vec::new(),
            version: 0,
        }
    }
    /// Adds an inclusive axis-aligned node blocker.
    pub fn block_box(&mut self, min: [f32; 3], max: [f32; 3]) -> ObstacleId {
        let id = ObstacleId(self.obstacles.len() as u32);
        self.obstacles.push(Some(Shape::Box(min, max)));
        self.version += 1;
        id
    }
    /// Adds a circular blocker in the x/z plane.
    pub fn block_circle(&mut self, center: [f32; 3], radius: f32) -> ObstacleId {
        let id = ObstacleId(self.obstacles.len() as u32);
        self.obstacles.push(Some(Shape::Circle(center, radius)));
        self.version += 1;
        id
    }
    /// Removes an obstacle, returning whether it existed.
    pub fn remove_obstacle(&mut self, id: ObstacleId) -> bool {
        remove(&mut self.obstacles, id.0, &mut self.version)
    }
    /// Adds an inclusive destination cost volume.
    pub fn add_cost_volume(&mut self, min: [f32; 3], max: [f32; 3], multiplier: f32) -> VolumeId {
        let id = VolumeId(self.volumes.len() as u32);
        self.volumes.push(Some(Volume {
            min,
            max,
            multiplier,
        }));
        self.version += 1;
        id
    }
    /// Removes a cost volume, returning whether it existed.
    pub fn remove_cost_volume(&mut self, id: VolumeId) -> bool {
        remove(&mut self.volumes, id.0, &mut self.version)
    }
    /// Returns the change version.
    pub fn version(&self) -> u64 {
        self.version
    }
    /// Returns the wrapped space.
    pub fn inner(&self) -> &S {
        &self.inner
    }
    /// Checks that all path nodes and consecutive edges remain traversable.
    pub fn path_is_valid(&self, path: &NodePath) -> bool {
        for (i, &n) in path.nodes.iter().enumerate() {
            if i > 0 && self.blocked(n) {
                return false;
            }
            if let Some(&next) = path.nodes.get(i + 1) {
                let mut edges = Vec::new();
                self.neighbours(n, &mut edges);
                if !edges.iter().any(|e| e.to == next) {
                    return false;
                }
            }
        }
        true
    }
    fn blocked(&self, node: NodeId) -> bool {
        let position = self.inner.position(node);
        self.obstacles.iter().flatten().any(|shape| match shape {
            Shape::Box(min, max) => inside(position, *min, *max),
            Shape::Circle(center, radius) => {
                let dx = position[0] - center[0];
                let dz = position[2] - center[2];
                dx * dx + dz * dz <= radius * radius
            }
        })
    }
}
impl<S: NavSpace> NavSpace for DynamicSpace<S> {
    fn node_count(&self) -> usize {
        self.inner.node_count()
    }
    fn position(&self, n: NodeId) -> [f32; 3] {
        self.inner.position(n)
    }
    fn area(&self, n: NodeId) -> AreaId {
        self.inner.area(n)
    }
    fn locate(&self, p: [f32; 3]) -> Option<NodeId> {
        self.inner.locate(p)
    }
    fn neighbours(&self, node: NodeId, out: &mut Vec<Edge>) {
        let mut edges = Vec::new();
        self.inner.neighbours(node, &mut edges);
        for mut edge in edges {
            if self.blocked(edge.to) {
                continue;
            }
            let p = self.inner.position(edge.to);
            for v in self.volumes.iter().flatten() {
                if inside(p, v.min, v.max) {
                    edge.length *= v.multiplier;
                }
            }
            out.push(edge);
        }
    }
}
fn remove<T>(items: &mut [Option<T>], id: u32, version: &mut u64) -> bool {
    if let Some(slot) = items.get_mut(id as usize)
        && slot.take().is_some()
    {
        *version += 1;
        return true;
    }
    false
}
fn inside(p: [f32; 3], a: [f32; 3], b: [f32; 3]) -> bool {
    (0..3).all(|i| p[i] >= a[i] && p[i] <= b[i])
}
