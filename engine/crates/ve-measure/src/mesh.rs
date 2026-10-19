//! The borrowed mesh view and the axis-aligned box, shared by every measurement.

use glam::Vec3;

use crate::error::MeasureError;

/// Area below which a triangle counts as degenerate, in square metres.
pub(crate) const AREA_EPSILON: f32 = 1e-10;
/// Edge length below which an edge counts as zero-length, in metres.
pub(crate) const LENGTH_EPSILON: f32 = 1e-7;
/// Volume below which the mesh counts as having no orientation, in cubic metres.
pub(crate) const VOLUME_EPSILON: f32 = 1e-12;

/// A read-only triangle mesh borrowed from plain arrays.
///
/// `positions` are in metres; `indices` are triangle corners into `positions` and the length is a
/// multiple of three; `uvs`, when present, hold one `[u, v]` per position.
#[derive(Clone, Copy, Debug)]
pub struct Mesh<'a> {
    /// Vertex positions, in metres.
    pub positions: &'a [[f32; 3]],
    /// Triangle indices; the length is a multiple of three.
    pub indices: &'a [u32],
    /// Per-vertex texture coordinates, one per position, or `None`.
    pub uvs: Option<&'a [[f32; 2]]>,
}

impl<'a> Mesh<'a> {
    /// A mesh with positions and triangle indices and no uvs.
    #[must_use]
    pub fn new(positions: &'a [[f32; 3]], indices: &'a [u32]) -> Self {
        Self {
            positions,
            indices,
            uvs: None,
        }
    }

    /// A mesh that also carries per-vertex uvs.
    #[must_use]
    pub fn with_uvs(positions: &'a [[f32; 3]], indices: &'a [u32], uvs: &'a [[f32; 2]]) -> Self {
        Self {
            positions,
            indices,
            uvs: Some(uvs),
        }
    }

    /// How many vertices the mesh declares.
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    /// How many triangles the mesh has, after checking it is well formed.
    ///
    /// # Errors
    /// Returns [`MeasureError`] when the mesh is empty, its indices are not a whole number of
    /// triangles or point out of range, a coordinate is non-finite, or the uvs do not match.
    pub fn triangle_count(&self) -> Result<usize, MeasureError> {
        self.validate()
    }

    pub(crate) fn validate(&self) -> Result<usize, MeasureError> {
        if !self.indices.len().is_multiple_of(3) {
            return Err(MeasureError::NotTriangles(self.indices.len()));
        }
        if self.indices.is_empty() {
            return Err(MeasureError::Empty);
        }
        for (i, p) in self.positions.iter().enumerate() {
            if !p.iter().all(|c| c.is_finite()) {
                return Err(MeasureError::NonFinite(i));
            }
        }
        for &index in self.indices {
            if index as usize >= self.positions.len() {
                return Err(MeasureError::IndexOutOfRange {
                    index,
                    vertices: self.positions.len(),
                });
            }
        }
        if let Some(uvs) = self.uvs
            && uvs.len() != self.positions.len()
        {
            return Err(MeasureError::UvLength {
                uvs: uvs.len(),
                vertices: self.positions.len(),
            });
        }
        Ok(self.indices.len() / 3)
    }

    pub(crate) fn triangle(&self, t: usize) -> [u32; 3] {
        [
            self.indices[t * 3],
            self.indices[t * 3 + 1],
            self.indices[t * 3 + 2],
        ]
    }

    pub(crate) fn position(&self, i: u32) -> Vec3 {
        Vec3::from_array(self.positions[i as usize])
    }

    pub(crate) fn corners(&self, t: usize) -> (Vec3, Vec3, Vec3) {
        let [a, b, c] = self.triangle(t);
        (self.position(a), self.position(b), self.position(c))
    }

    /// The axis-aligned bounding box of the vertices.
    pub(crate) fn bounds(&self) -> BoundingBox {
        let mut bounds = BoundingBox::empty();
        for p in self.positions {
            bounds.include(Vec3::from_array(*p));
        }
        bounds
    }

    /// The length of the bounding-box diagonal, a scale for epsilons, in metres.
    pub(crate) fn diagonal(&self) -> f32 {
        let bounds = self.bounds();
        Vec3::from_array(bounds.size()).length()
    }

    /// The signed volume of the surface, positive when the faces wind outward.
    pub(crate) fn signed_volume(&self) -> f32 {
        (0..self.indices.len() / 3)
            .map(|t| {
                let (a, b, c) = self.corners(t);
                a.dot(b.cross(c)) / 6.0
            })
            .sum()
    }
}

/// An axis-aligned box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundingBox {
    /// Lower corner, in metres.
    pub min: [f32; 3],
    /// Upper corner, in metres.
    pub max: [f32; 3],
}

impl BoundingBox {
    /// A box that includes nothing (inverted, ready to be grown).
    #[must_use]
    pub(crate) fn empty() -> Self {
        Self {
            min: [f32::INFINITY; 3],
            max: [f32::NEG_INFINITY; 3],
        }
    }

    pub(crate) fn include(&mut self, p: Vec3) {
        self.min = Vec3::from_array(self.min).min(p).to_array();
        self.max = Vec3::from_array(self.max).max(p).to_array();
    }

    /// The side lengths, in metres.
    #[must_use]
    pub fn size(&self) -> [f32; 3] {
        [
            self.max[0] - self.min[0],
            self.max[1] - self.min[1],
            self.max[2] - self.min[2],
        ]
    }

    /// The centre, in metres.
    #[must_use]
    pub fn centre(&self) -> [f32; 3] {
        [
            0.5 * (self.min[0] + self.max[0]),
            0.5 * (self.min[1] + self.max[1]),
            0.5 * (self.min[2] + self.max[2]),
        ]
    }

    pub(crate) fn centre_vec(&self) -> Vec3 {
        Vec3::from_array(self.centre())
    }
}

/// A tiny union-find over indices, used for connected components and UV islands.
pub(crate) struct Dsu {
    parent: Vec<u32>,
}

impl Dsu {
    pub(crate) fn new(n: usize) -> Self {
        Self {
            parent: (0..n as u32).collect(),
        }
    }

    pub(crate) fn find(&mut self, x: u32) -> u32 {
        let mut root = x;
        while self.parent[root as usize] != root {
            root = self.parent[root as usize];
        }
        let mut current = x;
        while self.parent[current as usize] != root {
            let next = self.parent[current as usize];
            self.parent[current as usize] = root;
            current = next;
        }
        root
    }

    pub(crate) fn union(&mut self, a: u32, b: u32) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.parent[ra as usize] = rb;
        }
    }
}
