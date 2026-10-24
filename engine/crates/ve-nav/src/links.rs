//! Off-mesh navigation links layered over any navigation space.

use crate::{AreaId, Edge, EdgeKind, NavError, NavSpace, NodeId};

/// Stable id of a link.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LinkId(pub u32);
/// Whether a link currently permits traversal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LinkState {
    /// Traversal is enabled.
    Open,
    /// Traversal is disabled.
    Closed,
}
/// A link endpoint, either a node or a point snapped on insertion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LinkEnd {
    /// A node in the wrapped space.
    Node(NodeId),
    /// A point located in the wrapped space.
    Point([f32; 3]),
}
/// Link definition.
#[derive(Clone, Debug)]
pub struct Link {
    /// Start endpoint.
    pub from: LinkEnd,
    /// End endpoint.
    pub to: LinkEnd,
    /// Traversal kind.
    pub kind: EdgeKind,
    /// Optional explicit length.
    pub length: Option<f32>,
    /// Whether traversal is only from `from` to `to`.
    pub one_way: bool,
    /// Optional lookup tag.
    pub tag: String,
}
#[derive(Clone, Debug)]
struct Entry {
    from: NodeId,
    to: NodeId,
    kind: EdgeKind,
    length: Option<f32>,
    one_way: bool,
    tag: String,
    state: LinkState,
}
/// A navigation space with deterministic, mutable off-mesh links.
pub struct LinkedSpace<S: NavSpace> {
    inner: S,
    links: Vec<Entry>,
    version: u64,
}

impl<S: NavSpace> LinkedSpace<S> {
    /// Wraps a navigation space.
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            links: Vec::new(),
            version: 0,
        }
    }
    /// Adds a link, snapping point endpoints through `locate`.
    ///
    /// # Errors
    /// Returns `UnknownNode` when either endpoint is invalid.
    pub fn add_link(&mut self, link: Link) -> Result<LinkId, NavError> {
        let from = self.resolve(link.from)?;
        let to = self.resolve(link.to)?;
        let id =
            LinkId(u32::try_from(self.links.len()).map_err(|_| NavError::UnknownLink(u32::MAX))?);
        self.links.push(Entry {
            from,
            to,
            kind: link.kind,
            length: link.length,
            one_way: link.one_way,
            tag: link.tag,
            state: LinkState::Open,
        });
        self.version += 1;
        Ok(id)
    }
    fn resolve(&self, end: LinkEnd) -> Result<NodeId, NavError> {
        let node = match end {
            LinkEnd::Node(n) => Some(n),
            LinkEnd::Point(p) => self.inner.locate(p),
        };
        match node {
            Some(n) if (n as usize) < self.inner.node_count() => Ok(n),
            Some(n) => Err(NavError::UnknownNode(n)),
            None => Err(NavError::UnknownNode(u32::MAX)),
        }
    }
    /// Changes a link state and bumps the version only when it changes.
    ///
    /// # Errors
    /// Returns `UnknownLink` when the id does not exist.
    pub fn set_link_state(&mut self, id: LinkId, state: LinkState) -> Result<(), NavError> {
        let e = self
            .links
            .get_mut(id.0 as usize)
            .ok_or(NavError::UnknownLink(id.0))?;
        if e.state != state {
            e.state = state;
            self.version += 1;
        }
        Ok(())
    }
    /// Returns a link state if the id exists.
    pub fn link_state(&self, id: LinkId) -> Option<LinkState> {
        self.links.get(id.0 as usize).map(|e| e.state)
    }
    /// Finds the first non-empty matching tag.
    pub fn link_by_tag(&self, tag: &str) -> Option<LinkId> {
        self.links
            .iter()
            .position(|e| !e.tag.is_empty() && e.tag == tag)
            .and_then(|i| u32::try_from(i).ok())
            .map(LinkId)
    }
    /// Returns the change version.
    pub fn version(&self) -> u64 {
        self.version
    }
    /// Returns the wrapped space.
    pub fn inner(&self) -> &S {
        &self.inner
    }
}
impl<S: NavSpace> NavSpace for LinkedSpace<S> {
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
        self.inner.neighbours(node, out);
        for e in &self.links {
            if e.state != LinkState::Open {
                continue;
            }
            let length = e
                .length
                .unwrap_or_else(|| distance(self.position(e.from), self.position(e.to)));
            if e.from == node {
                out.push(Edge {
                    to: e.to,
                    length,
                    kind: e.kind,
                });
            }
            if !e.one_way && e.to == node {
                out.push(Edge {
                    to: e.from,
                    length,
                    kind: e.kind,
                });
            }
        }
    }
}
fn distance(from: [f32; 3], to: [f32; 3]) -> f32 {
    let dx = from[0] - to[0];
    let dy = from[1] - to[1];
    let dz = from[2] - to[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}
