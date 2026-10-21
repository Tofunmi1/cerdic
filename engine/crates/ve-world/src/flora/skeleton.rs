//! The skeleton of a plant: nodes in a tree, with the data a mesher and a wind shader need.

/// A leaf (or flower, or needle cluster) attached to a node.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Leaf {
    /// Where it attaches, in the plant's own space (base at the origin, +y up).
    pub position: [f32; 3],
    /// Unit direction the leaf points (its tip).
    pub direction: [f32; 3],
    /// Length in metres.
    pub size: f32,
    /// The skeleton node it grows from.
    pub node: u32,
}

/// A plant as parallel arrays, one entry per node. Node 0 is the root at the origin; every other node's parent has a smaller index.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Skeleton {
    /// Node positions in metres.
    pub position: Vec<[f32; 3]>,
    /// Radius of the branch at each node.
    pub radius: Vec<f32>,
    /// Parent node, `u32::MAX` for the root.
    pub parent: Vec<u32>,
    /// Branch depth: 0 trunk, 1 first branches, and so on.
    pub depth: Vec<u8>,
    /// Wind amplitude weight 0..=1: grows with distance from the base and with depth (two-level sway).
    pub wind: Vec<f32>,
    /// Per-branch gust phase 0..1; every node of one branch shares it.
    pub phase: Vec<f32>,
    /// Stiffness 0..1: 1 at the trunk base falling to 0.1 at twig tips; the wind amplitude of a node is `wind * (1 - stiffness)`.
    pub stiffness: Vec<f32>,
    /// Branch id of each node (the polyline it belongs to).
    pub branch: Vec<u32>,
    /// Leaves.
    pub leaves: Vec<Leaf>,
}

impl Skeleton {
    /// Number of nodes.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.position.len()
    }

    /// The highest point, in metres above the origin.
    #[must_use]
    pub fn height(&self) -> f32 {
        self.position.iter().fold(0.0, |top, p| top.max(p[1]))
    }

    /// Each branch as a chain of node indices from where it forks off its parent to its tip, trunk first. A branch's chain starts with
    /// the node it grew from (when it has one), then its own nodes in order.
    #[must_use]
    pub fn polylines(&self) -> Vec<Vec<u32>> {
        let mut ids: Vec<u32> = self.branch.clone();
        ids.sort_unstable();
        ids.dedup();
        ids.into_iter()
            .map(|id| {
                let own: Vec<u32> = (0..self.branch.len())
                    .filter(|&node| self.branch[node] == id)
                    .map(|node| u32::try_from(node).unwrap_or(u32::MAX))
                    .collect();
                let mut chain = Vec::with_capacity(own.len() + 1);
                if let Some(&first) = own.first() {
                    let fork = self.parent[first as usize];
                    if fork != u32::MAX && self.branch[fork as usize] != id {
                        chain.push(fork);
                    }
                }
                chain.extend(own);
                chain
            })
            .collect()
    }
}
