//! Shared editable polygon soup for the native operators: faces as vertex lists, with provenance.

use std::collections::BTreeMap;

use crate::{BeveledMesh, KernelError, PolygonMesh};

/// Marks an element that descends from no input element.
pub const NO_ORIGIN: u32 = BeveledMesh::NO_ORIGIN;

/// A polygon mesh that operators rebuild freely, carrying where each element came from.
#[derive(Clone, Debug, Default)]
pub struct Soup {
    /// Vertex positions.
    pub positions: Vec<[f32; 3]>,
    /// Each face's vertex indices, in winding order.
    pub faces: Vec<Vec<u32>>,
    /// For each vertex, the input vertex whose attributes it inherits (or [`NO_ORIGIN`]).
    pub vertex_origin: Vec<u32>,
    /// For each face, the input face whose attributes it inherits (or [`NO_ORIGIN`]).
    pub face_parent: Vec<u32>,
    /// What the operator made each face (see [`BeveledMesh::face_role`]).
    pub face_role: Vec<u32>,
}

impl Soup {
    /// Copies a borrowed mesh; every element is its own origin and parent.
    ///
    /// # Errors
    /// [`KernelError::InvalidArgument`] for a face with fewer than three corners or sizes that do not add up to the
    /// corners, [`KernelError::OutOfBounds`] for an index past the vertices, [`KernelError::NonFinite`] for NaN or
    /// infinite coordinates.
    pub fn from_mesh(mesh: &PolygonMesh<'_>) -> Result<Self, KernelError> {
        if mesh.positions.iter().flatten().any(|v| !v.is_finite()) {
            return Err(KernelError::NonFinite);
        }
        let mut faces = Vec::with_capacity(mesh.face_sizes.len());
        let mut at = 0_usize;
        for &size in mesh.face_sizes {
            let size = size as usize;
            if size < 3 || at + size > mesh.corners.len() {
                return Err(KernelError::InvalidArgument(
                    "a face needs at least three corners that exist",
                ));
            }
            let face = mesh.corners[at..at + size].to_vec();
            if face.iter().any(|&v| v as usize >= mesh.positions.len()) {
                return Err(KernelError::OutOfBounds);
            }
            faces.push(face);
            at += size;
        }
        if at != mesh.corners.len() {
            return Err(KernelError::InvalidArgument(
                "face sizes do not cover the corners",
            ));
        }
        let vertex_count =
            u32::try_from(mesh.positions.len()).map_err(|_| KernelError::TooLarge)?;
        let face_count = u32::try_from(faces.len()).map_err(|_| KernelError::TooLarge)?;
        Ok(Self {
            positions: mesh.positions.to_vec(),
            vertex_origin: (0..vertex_count).collect(),
            face_parent: (0..face_count).collect(),
            face_role: vec![0; faces.len()],
            faces,
        })
    }

    /// Adds a vertex and returns its index.
    pub fn add_vertex(&mut self, position: [f32; 3], origin: u32) -> u32 {
        self.positions.push(position);
        self.vertex_origin.push(origin);
        u32::try_from(self.positions.len() - 1).unwrap_or(u32::MAX)
    }

    /// Adds a face and returns its index.
    pub fn add_face(&mut self, corners: Vec<u32>, parent: u32, role: u32) -> usize {
        self.faces.push(corners);
        self.face_parent.push(parent);
        self.face_role.push(role);
        self.faces.len() - 1
    }

    /// Drops vertices no face uses (keeping the order of the rest) and returns the finished mesh.
    #[must_use]
    pub fn finish(self) -> BeveledMesh {
        let mut remap = vec![u32::MAX; self.positions.len()];
        let mut positions = Vec::new();
        let mut vertex_origin = Vec::new();
        let mut face_sizes = Vec::with_capacity(self.faces.len());
        let mut corners = Vec::new();
        for face in &self.faces {
            face_sizes.push(u32::try_from(face.len()).unwrap_or(u32::MAX));
            for &v in face {
                let slot = &mut remap[v as usize];
                if *slot == u32::MAX {
                    *slot = u32::try_from(positions.len()).unwrap_or(u32::MAX);
                    positions.push(self.positions[v as usize]);
                    vertex_origin.push(self.vertex_origin[v as usize]);
                }
                corners.push(*slot);
            }
        }
        let mut face_parent = self.face_parent;
        inherit_origins(&positions, &face_sizes, &corners, &mut vertex_origin, &mut face_parent);
        BeveledMesh {
            positions,
            face_sizes,
            corners,
            vertex_origin,
            face_parent,
            face_role: self.face_role,
        }
    }

    /// The faces on each undirected edge `(low, high)`, as `(face, corner index of the edge's first vertex)`.
    #[must_use]
    pub fn edge_faces(&self) -> BTreeMap<(u32, u32), Vec<(usize, usize)>> {
        let mut map: BTreeMap<(u32, u32), Vec<(usize, usize)>> = BTreeMap::new();
        for (f, face) in self.faces.iter().enumerate() {
            for i in 0..face.len() {
                let (a, b) = (face[i], face[(i + 1) % face.len()]);
                map.entry((a.min(b), a.max(b))).or_default().push((f, i));
            }
        }
        map
    }

    /// How far to push a point at fraction `t` along the edge `a`-`b` so a cut edge follows a smooth surface: along the
    /// two faces' average normal, by `smooth` times the height of the circular arc that turns by the angle between
    /// the faces (`L / 2 * tan(angle / 4)`, scaled by the parabola `4 t (1 - t)`). Zero on boundary edges, flat
    /// edges and concave edges. `users` are the edge's faces from [`Soup::edge_faces`].
    #[must_use]
    pub fn arc_push(&self, users: &[(usize, usize)], a: u32, b: u32, t: f64, smooth: f64) -> [f64; 3] {
        let [(f, _), (g, _)] = users else {
            return [0.0; 3];
        };
        if smooth == 0.0 {
            return [0.0; 3];
        }
        let unit = |n: [f64; 3]| {
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            (l > 1e-20).then(|| [n[0] / l, n[1] / l, n[2] / l])
        };
        let (Some(n1), Some(n2)) = (unit(self.face_normal(*f)), unit(self.face_normal(*g))) else {
            return [0.0; 3];
        };
        let dot = n1[0] * n2[0] + n1[1] * n2[1] + n1[2] * n2[2];
        let sum = [n1[0] + n2[0], n1[1] + n2[1], n1[2] + n2[2]];
        let Some(direction) = unit(sum) else {
            return [0.0; 3];
        };
        let (pa, pb) = (self.positions[a as usize], self.positions[b as usize]);
        let edge = [f64::from(pb[0] - pa[0]), f64::from(pb[1] - pa[1]), f64::from(pb[2] - pa[2])];
        // Convex when the second face bends away from the first's plane (its far corner lies behind it).
        let far = self.faces[*g]
            .iter()
            .map(|&v| {
                let p = self.positions[v as usize];
                (f64::from(p[0] - pa[0]) * n1[0] + f64::from(p[1] - pa[1]) * n1[1] + f64::from(p[2] - pa[2]) * n1[2], v)
            })
            .filter(|&(_, v)| v != a && v != b)
            .map(|(d, _)| d)
            .fold(0.0_f64, |m, d| if d.abs() > m.abs() { d } else { m });
        if far >= 0.0 {
            return [0.0; 3];
        }
        let length = (edge[0] * edge[0] + edge[1] * edge[1] + edge[2] * edge[2]).sqrt();
        let turn = dot.clamp(-1.0, 1.0).acos();
        let push = smooth * (length / 2.0) * (turn / 4.0).tan() * 4.0 * t * (1.0 - t);
        [direction[0] * push, direction[1] * push, direction[2] * push]
    }

    /// Newell normal of a face (not normalised; its length is twice the face's area).
    #[must_use]
    pub fn face_normal(&self, face: usize) -> [f64; 3] {
        let f = &self.faces[face];
        let mut n = [0.0_f64; 3];
        for i in 0..f.len() {
            let a = self.positions[f[i] as usize];
            let b = self.positions[f[(i + 1) % f.len()] as usize];
            n[0] += (f64::from(a[1]) - f64::from(b[1])) * (f64::from(a[2]) + f64::from(b[2]));
            n[1] += (f64::from(a[2]) - f64::from(b[2])) * (f64::from(a[0]) + f64::from(b[0]));
            n[2] += (f64::from(a[0]) - f64::from(b[0])) * (f64::from(a[1]) + f64::from(b[1]));
        }
        n
    }
}

fn distance_squared(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]) * (a[i] - b[i])).sum()
}

/// Gives every vertex and face that descends from nothing the origin of the nearest element that does, so a caller that copies attributes by origin always
/// has a source. A new vertex takes it from the closest vertex of a face it belongs to (else the closest vertex anywhere); a new face takes the parent of a
/// face that shares an edge with it (else the face whose centre is closest). With no claimed element at all the first input element stands in.
fn inherit_origins(positions: &[[f32; 3]], face_sizes: &[u32], corners: &[u32], vertex_origin: &mut [u32], face_parent: &mut [u32]) {
    let mut starts = Vec::with_capacity(face_sizes.len());
    let mut at = 0_usize;
    for &size in face_sizes {
        starts.push(at);
        at += size as usize;
    }
    let face = |f: usize| &corners[starts[f]..starts[f] + face_sizes[f] as usize];

    if vertex_origin.contains(&NO_ORIGIN) {
        let claimed = vertex_origin.to_vec();
        let mut faces_of = vec![Vec::new(); positions.len()];
        for f in 0..face_sizes.len() {
            for &v in face(f) {
                faces_of[v as usize].push(f);
            }
        }
        for v in 0..positions.len() {
            if claimed[v] != NO_ORIGIN {
                continue;
            }
            let here = positions[v];
            let nearest = |pool: &mut dyn Iterator<Item = usize>| {
                pool.filter(|&w| claimed[w] != NO_ORIGIN)
                    .min_by(|&a, &b| distance_squared(here, positions[a]).total_cmp(&distance_squared(here, positions[b])))
            };
            let source = nearest(&mut faces_of[v].iter().flat_map(|&f| face(f).iter().map(|&w| w as usize)))
                .or_else(|| nearest(&mut (0..positions.len())));
            vertex_origin[v] = source.map_or(0, |w| claimed[w]);
        }
    }

    if face_parent.contains(&NO_ORIGIN) {
        let mut neighbours: BTreeMap<(u32, u32), Vec<usize>> = BTreeMap::new();
        for f in 0..face_sizes.len() {
            let corners = face(f);
            for i in 0..corners.len() {
                let (a, b) = (corners[i], corners[(i + 1) % corners.len()]);
                neighbours.entry((a.min(b), a.max(b))).or_default().push(f);
            }
        }
        // A new face next to another new face is settled on a later pass, once its neighbour has a parent.
        loop {
            let mut changed = false;
            for f in 0..face_sizes.len() {
                if face_parent[f] != NO_ORIGIN {
                    continue;
                }
                let corners = face(f);
                let parent = (0..corners.len())
                    .flat_map(|i| neighbours[&(corners[i].min(corners[(i + 1) % corners.len()]), corners[i].max(corners[(i + 1) % corners.len()]))].iter())
                    .find(|&&other| other != f && face_parent[other] != NO_ORIGIN)
                    .map(|&other| face_parent[other]);
                if let Some(parent) = parent {
                    face_parent[f] = parent;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        let centre = |f: usize| {
            let corners = face(f);
            let mut sum = [0.0_f32; 3];
            for &v in corners {
                for (slot, value) in sum.iter_mut().zip(positions[v as usize]) {
                    *slot += value;
                }
            }
            sum.map(|value| value / corners.len() as f32)
        };
        let claimed = face_parent.to_vec();
        for f in 0..face_sizes.len() {
            if claimed[f] != NO_ORIGIN || face_parent[f] != NO_ORIGIN {
                continue;
            }
            let here = centre(f);
            face_parent[f] = (0..face_sizes.len())
                .filter(|&other| claimed[other] != NO_ORIGIN)
                .min_by(|&a, &b| distance_squared(here, centre(a)).total_cmp(&distance_squared(here, centre(b))))
                .map_or(0, |other| claimed[other]);
        }
    }
}
