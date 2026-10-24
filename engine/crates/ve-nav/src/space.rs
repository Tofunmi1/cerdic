//! Open navigation-space interfaces and identifiers.

/// Stable identifier for a node in a navigation space.
pub type NodeId = u32;

/// Area annotation attached to a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AreaId(pub u8);

impl AreaId {
    /// Default ground area.
    pub const GROUND: Self = Self(0);
    /// Largest supported area id.
    pub const MAX: u8 = 63;
    /// Creates an area id when it fits the supported range.
    #[must_use]
    pub const fn new(id: u8) -> Option<Self> {
        if id <= Self::MAX {
            Some(Self(id))
        } else {
            None
        }
    }
}

/// Traversal kind for an edge. Values 16 and above are reserved for game-defined kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EdgeKind(pub u8);

impl EdgeKind {
    /// Walking traversal.
    pub const WALK: Self = Self(0);
    /// Jump traversal.
    pub const JUMP: Self = Self(1);
    /// Climbing traversal.
    pub const CLIMB: Self = Self(2);
    /// Swimming traversal.
    pub const SWIM: Self = Self(3);
    /// Flying traversal.
    pub const FLY: Self = Self(4);
    /// Door traversal.
    pub const DOOR: Self = Self(5);
    /// Teleport traversal.
    pub const TELEPORT: Self = Self(6);
}

/// Directed connection from a node to a neighbour.
#[derive(Clone, Copy, Debug)]
pub struct Edge {
    /// Destination node.
    pub to: NodeId,
    /// Geometric length of the connection.
    pub length: f32,
    /// Traversal kind.
    pub kind: EdgeKind,
}

/// A deterministic graph-like navigable space.
pub trait NavSpace {
    /// Number of nodes, whose ids are in `0..node_count()`.
    fn node_count(&self) -> usize;
    /// Position of a node.
    fn position(&self, node: NodeId) -> [f32; 3];
    /// Area annotation for a node.
    fn area(&self, node: NodeId) -> AreaId;
    /// Appends outgoing edges in deterministic order; does not clear `out`.
    fn neighbours(&self, node: NodeId, out: &mut Vec<Edge>);
    /// Finds the node nearest a point, if any.
    fn locate(&self, point: [f32; 3]) -> Option<NodeId>;
}
