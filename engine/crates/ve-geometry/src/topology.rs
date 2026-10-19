//! Deterministic mesh adjacency derived from polygon corner loops.

mod boundary;

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use ve_kernel::ExecutionContext;

use crate::{GeometryError, PolyMesh};

/// Classification of an undirected edge by its incident face count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeKind {
    /// Exactly one face uses the edge.
    Boundary,
    /// Exactly two faces use the edge.
    Manifold,
    /// Three or more faces use the edge.
    NonManifold(usize),
}

/// One incident face and the direction in which its loop traverses an edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EdgeUse {
    /// Face ordinal in this input mesh.
    pub face: usize,
    /// `true` when the face traverses the sorted edge from low to high vertex id.
    pub forward: bool,
}

/// Faces incident to one sorted vertex pair.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EdgeInfo {
    uses: Vec<EdgeUse>,
}

impl EdgeInfo {
    /// Face incidences in stable face and corner traversal order.
    #[must_use]
    pub fn uses(&self) -> &[EdgeUse] {
        &self.uses
    }

    /// Number of incident faces.
    #[must_use]
    pub fn face_count(&self) -> usize {
        self.uses.len()
    }

    /// Edge incidence class.
    #[must_use]
    pub fn kind(&self) -> EdgeKind {
        match self.uses.len() {
            0 | 1 => EdgeKind::Boundary,
            2 => EdgeKind::Manifold,
            count => EdgeKind::NonManifold(count),
        }
    }
}

/// Deterministic adjacency and topological summaries for a `PolyMesh`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Topology {
    edges: BTreeMap<(u32, u32), EdgeInfo>,
    face_neighbors: Vec<Vec<usize>>,
    vertex_faces: Vec<Vec<usize>>,
    components: Vec<Vec<u32>>,
    isolated_vertices: Vec<u32>,
    boundary_loops: Vec<Vec<u32>>,
    euler_characteristic: i64,
}

impl Topology {
    /// Builds edge, face and vertex adjacency in fixed order, observing cancellation.
    ///
    /// # Errors
    /// Returns [`GeometryError::Cancelled`] when cancellation was requested.
    pub fn build(mesh: &PolyMesh, context: &ExecutionContext) -> Result<Self, GeometryError> {
        let mut edges = BTreeMap::<(u32, u32), EdgeInfo>::new();
        let mut vertex_faces = vec![Vec::new(); mesh.positions().len()];
        let mut corner_start = 0_usize;
        for (face_id, size) in mesh.face_sizes().iter().copied().enumerate() {
            check_cancelled(context)?;
            let size = usize::try_from(size)
                .map_err(|_| GeometryError::InvalidMesh("face size overflows usize"))?;
            let face = mesh
                .corners()
                .get(corner_start..corner_start + size)
                .ok_or(GeometryError::InvalidMesh("face range is outside corners"))?;
            for (corner, &a) in face.iter().enumerate() {
                let b = face[(corner + 1) % face.len()];
                let key = (a.min(b), a.max(b));
                edges
                    .entry(key)
                    .or_insert_with(|| EdgeInfo { uses: Vec::new() })
                    .uses
                    .push(EdgeUse {
                        face: face_id,
                        forward: a < b,
                    });
                let adjacent =
                    vertex_faces
                        .get_mut(usize::try_from(a).map_err(|_| {
                            GeometryError::InvalidMesh("vertex index overflows usize")
                        })?)
                        .ok_or(GeometryError::InvalidMesh(
                            "corner refers to missing vertex",
                        ))?;
                if adjacent.last() != Some(&face_id) {
                    adjacent.push(face_id);
                }
            }
            corner_start += size;
        }
        let face_neighbors = face_adjacency(mesh.face_count(), &edges, context)?;
        let (components, isolated_vertices) =
            vertex_components(mesh.positions().len(), &edges, context)?;
        let boundary_loops = boundary::closed_loops(&edges, context)?;
        let vertices = i64::try_from(mesh.positions().len())
            .map_err(|_| GeometryError::InvalidMesh("vertex count exceeds Euler range"))?;
        let edge_count = i64::try_from(edges.len())
            .map_err(|_| GeometryError::InvalidMesh("edge count exceeds Euler range"))?;
        let faces = i64::try_from(mesh.face_count())
            .map_err(|_| GeometryError::InvalidMesh("face count exceeds Euler range"))?;
        let euler_characteristic = vertices - edge_count + faces;
        Ok(Self {
            edges,
            face_neighbors,
            vertex_faces,
            components,
            isolated_vertices,
            boundary_loops,
            euler_characteristic,
        })
    }

    /// Sorted edge map keyed by `(min_vertex, max_vertex)`.
    #[must_use]
    pub fn edges(&self) -> &BTreeMap<(u32, u32), EdgeInfo> {
        &self.edges
    }
    /// Face adjacency, sorted and duplicate-free per face.
    #[must_use]
    pub fn face_neighbors(&self) -> &[Vec<usize>] {
        &self.face_neighbors
    }
    /// Face incidence for each vertex.
    #[must_use]
    pub fn vertex_faces(&self) -> &[Vec<usize>] {
        &self.vertex_faces
    }
    /// Connected vertex components that contain at least one edge, sorted by smallest vertex id.
    #[must_use]
    pub fn components(&self) -> &[Vec<u32>] {
        &self.components
    }
    /// Vertices that are not referenced by any face.
    #[must_use]
    pub fn isolated_vertices(&self) -> &[u32] {
        &self.isolated_vertices
    }
    /// Closed directed boundary loops, each rotated to start at its smallest vertex id.
    #[must_use]
    pub fn boundary_loops(&self) -> &[Vec<u32>] {
        &self.boundary_loops
    }
    /// `V - E + F`, including isolated vertices.
    #[must_use]
    pub const fn euler_characteristic(&self) -> i64 {
        self.euler_characteristic
    }
}

fn face_adjacency(
    face_count: usize,
    edges: &BTreeMap<(u32, u32), EdgeInfo>,
    context: &ExecutionContext,
) -> Result<Vec<Vec<usize>>, GeometryError> {
    let mut neighbors = vec![BTreeSet::new(); face_count];
    for (index, info) in edges.values().enumerate() {
        if index % 1024 == 0 {
            check_cancelled(context)?;
        }
        if info.face_count() != 2 {
            continue;
        }
        let [a, b] = [info.uses[0].face, info.uses[1].face];
        neighbors[a].insert(b);
        neighbors[b].insert(a);
    }
    Ok(neighbors
        .into_iter()
        .map(|set| set.into_iter().collect())
        .collect())
}

fn vertex_components(
    vertex_count: usize,
    edges: &BTreeMap<(u32, u32), EdgeInfo>,
    context: &ExecutionContext,
) -> Result<(Vec<Vec<u32>>, Vec<u32>), GeometryError> {
    let mut adjacency = vec![Vec::new(); vertex_count];
    for (index, &(a, b)) in edges.keys().enumerate() {
        if index % 1024 == 0 {
            check_cancelled(context)?;
        }
        let a = usize::try_from(a)
            .map_err(|_| GeometryError::InvalidMesh("vertex index overflows usize"))?;
        let b = usize::try_from(b)
            .map_err(|_| GeometryError::InvalidMesh("vertex index overflows usize"))?;
        adjacency[a].push(b);
        adjacency[b].push(a);
    }
    let mut visited = vec![false; vertex_count];
    let mut components = Vec::new();
    let mut isolated = Vec::new();
    for start in 0..vertex_count {
        if visited[start] {
            continue;
        }
        if adjacency[start].is_empty() {
            visited[start] = true;
            isolated.push(
                u32::try_from(start)
                    .map_err(|_| GeometryError::InvalidMesh("vertex count exceeds u32"))?,
            );
            continue;
        }
        let mut queue = VecDeque::from([start]);
        let mut component = Vec::new();
        visited[start] = true;
        while let Some(vertex) = queue.pop_front() {
            if component.len() % 1024 == 0 {
                check_cancelled(context)?;
            }
            component.push(
                u32::try_from(vertex)
                    .map_err(|_| GeometryError::InvalidMesh("vertex count exceeds u32"))?,
            );
            for &next in &adjacency[vertex] {
                if !visited[next] {
                    visited[next] = true;
                    queue.push_back(next);
                }
            }
        }
        component.sort_unstable();
        components.push(component);
    }
    Ok((components, isolated))
}

pub(crate) fn check_cancelled(context: &ExecutionContext) -> Result<(), GeometryError> {
    if context.is_cancelled() {
        Err(GeometryError::Cancelled)
    } else {
        Ok(())
    }
}
