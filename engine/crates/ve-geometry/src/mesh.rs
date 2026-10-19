//! Polygon meshes with attributes.

use std::collections::{BTreeMap, BTreeSet};

use crate::attr::{Attribute, Domain};
use crate::error::GeometryError;

/// Longest attribute name, in bytes.
pub const MAX_ATTRIBUTE_NAME: usize = 64;

/// A mesh of polygon faces. Faces may have any number of corners (three or more); a corner is one
/// vertex index, listed face after face.
#[derive(Clone, Debug, PartialEq)]
pub struct PolyMesh {
    positions: Vec<[f32; 3]>,
    face_sizes: Vec<u32>,
    corners: Vec<u32>,
    attributes: BTreeMap<String, Attribute>,
    triangulation: crate::TriangulationMode,
}

impl PolyMesh {
    /// Builds a mesh, checking that faces and indices are consistent and positions finite.
    ///
    /// # Errors
    /// [`GeometryError::InvalidMesh`] for a face of fewer than three corners, face sizes that do
    /// not add up to the corner count, an index outside the positions, or a non-finite position.
    pub fn new(
        positions: Vec<[f32; 3]>,
        face_sizes: Vec<u32>,
        corners: Vec<u32>,
    ) -> Result<Self, GeometryError> {
        if positions.iter().flatten().any(|v| !v.is_finite()) {
            return Err(GeometryError::InvalidMesh("a position is not finite"));
        }
        if face_sizes.iter().any(|&n| n < 3) {
            return Err(GeometryError::InvalidMesh(
                "a face has fewer than three corners",
            ));
        }
        let total: u64 = face_sizes.iter().map(|&n| u64::from(n)).sum();
        if total != corners.len() as u64 {
            return Err(GeometryError::InvalidMesh(
                "face sizes do not add up to the corner count",
            ));
        }
        let limit = positions.len();
        if corners
            .iter()
            .any(|&c| usize::try_from(c).map_or(true, |c| c >= limit))
        {
            return Err(GeometryError::InvalidMesh(
                "a corner refers to a missing vertex",
            ));
        }
        Ok(Self {
            positions,
            face_sizes,
            corners,
            attributes: BTreeMap::new(),
            triangulation: crate::TriangulationMode::PlanarOnly,
        })
    }

    /// Vertex positions.
    #[must_use]
    pub fn positions(&self) -> &[[f32; 3]] {
        &self.positions
    }

    /// Corners per face.
    #[must_use]
    pub fn face_sizes(&self) -> &[u32] {
        &self.face_sizes
    }

    /// Vertex index of every corner, face after face.
    #[must_use]
    pub fn corners(&self) -> &[u32] {
        &self.corners
    }

    /// All attributes, ordered by name.
    #[must_use]
    pub fn attributes(&self) -> &BTreeMap<String, Attribute> {
        &self.attributes
    }

    /// Triangulation policy used when deriving a triangle mesh from this polygon mesh.
    #[must_use]
    pub fn triangulation_mode(&self) -> crate::TriangulationMode {
        self.triangulation
    }

    /// Allows mildly curved quads to be triangulated for subdivision output.
    pub fn set_triangulation_mode(&mut self, mode: crate::TriangulationMode) {
        self.triangulation = mode;
    }

    /// The same faces with every position moved by `map` (a move, turn or stretch, say). Attributes and the triangulation policy
    /// carry over unchanged, so a vertex attribute that holds directions (normals) is not turned with the positions.
    ///
    /// # Errors
    /// [`GeometryError::InvalidMesh`] when `map` makes a position not finite.
    pub fn transformed(&self, map: impl Fn([f32; 3]) -> [f32; 3]) -> Result<Self, GeometryError> {
        let mut moved = Self::new(
            self.positions.iter().map(|p| map(*p)).collect(),
            self.face_sizes.clone(),
            self.corners.clone(),
        )?;
        moved.attributes.clone_from(&self.attributes);
        moved.triangulation = self.triangulation;
        Ok(moved)
    }

    /// Number of faces.
    #[must_use]
    pub fn face_count(&self) -> usize {
        self.face_sizes.len()
    }

    /// Number of undirected edges.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        let mut edges = BTreeSet::new();
        let mut start = 0_usize;
        for &size in &self.face_sizes {
            let size = size as usize;
            let face = self.corners.get(start..start + size).unwrap_or(&[]);
            for (i, &a) in face.iter().enumerate() {
                let b = face.get((i + 1) % size).copied().unwrap_or(a);
                edges.insert((a.min(b), a.max(b)));
            }
            start += size;
        }
        edges.len()
    }

    /// How many elements the domain has on this mesh, or `None` if a mesh has no such domain.
    #[must_use]
    pub fn domain_len(&self, domain: Domain) -> Option<usize> {
        match domain {
            Domain::Point => Some(self.positions.len()),
            Domain::Edge => Some(self.edge_count()),
            Domain::Face => Some(self.face_sizes.len()),
            Domain::Corner => Some(self.corners.len()),
            Domain::CurvePoint | Domain::Instance => None,
        }
    }

    /// Adds or replaces an attribute after checking its name and length against its domain.
    ///
    /// # Errors
    /// [`GeometryError::BadAttributeName`], [`GeometryError::DomainNotOnMesh`] or
    /// [`GeometryError::AttributeLength`].
    pub fn set_attribute(&mut self, name: &str, attribute: Attribute) -> Result<(), GeometryError> {
        if name.is_empty() || name.len() > MAX_ATTRIBUTE_NAME {
            return Err(GeometryError::BadAttributeName(name.to_owned()));
        }
        let expected = self
            .domain_len(attribute.domain)
            .ok_or(GeometryError::DomainNotOnMesh(attribute.domain))?;
        if attribute.data.len() != expected {
            return Err(GeometryError::AttributeLength {
                name: name.to_owned(),
                expected,
                found: attribute.data.len(),
            });
        }
        self.attributes.insert(name.to_owned(), attribute);
        Ok(())
    }

    /// Removes an attribute, returning it.
    pub fn remove_attribute(&mut self, name: &str) -> Option<Attribute> {
        self.attributes.remove(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attr::AttrData;

    fn cube() -> PolyMesh {
        let p = |x: f32, y: f32, z: f32| [x, y, z];
        let positions = vec![
            p(0.0, 0.0, 0.0),
            p(1.0, 0.0, 0.0),
            p(1.0, 1.0, 0.0),
            p(0.0, 1.0, 0.0),
            p(0.0, 0.0, 1.0),
            p(1.0, 0.0, 1.0),
            p(1.0, 1.0, 1.0),
            p(0.0, 1.0, 1.0),
        ];
        let corners = vec![
            0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 3, 7, 6, 2, 0, 4, 7, 3, 1, 2, 6, 5,
        ];
        PolyMesh::new(positions, vec![4; 6], corners).unwrap()
    }

    #[test]
    fn a_cube_satisfies_euler() {
        let m = cube();
        assert_eq!(
            (m.positions().len(), m.edge_count(), m.face_count()),
            (8, 12, 6)
        );
        assert_eq!(8 + 6 - 12, 2);
    }

    #[test]
    fn construction_refuses_broken_meshes() {
        let one = vec![[0.0; 3]; 3];
        assert!(PolyMesh::new(one.clone(), vec![2], vec![0, 1]).is_err());
        assert!(PolyMesh::new(one.clone(), vec![3], vec![0, 1]).is_err());
        assert!(PolyMesh::new(one.clone(), vec![3], vec![0, 1, 3]).is_err());
        let mut nan = one;
        nan[0][0] = f32::NAN;
        assert!(PolyMesh::new(nan, vec![3], vec![0, 1, 2]).is_err());
    }

    #[test]
    fn attributes_must_match_their_domain_length() {
        let mut m = cube();
        let weights = |n: usize| Attribute {
            domain: Domain::Point,
            data: AttrData::F32(vec![0.0; n]),
        };
        assert!(m.set_attribute("weight", weights(8)).is_ok());
        assert_eq!(
            m.set_attribute("weight", weights(7)),
            Err(GeometryError::AttributeLength {
                name: "weight".into(),
                expected: 8,
                found: 7
            })
        );
        let uv = Attribute {
            domain: Domain::Corner,
            data: AttrData::F32(vec![0.0; 24]),
        };
        assert!(m.set_attribute("u", uv).is_ok());
        let per_edge = Attribute {
            domain: Domain::Edge,
            data: AttrData::U32(vec![0; 12]),
        };
        assert!(m.set_attribute("crease", per_edge).is_ok());
        let curve = Attribute {
            domain: Domain::CurvePoint,
            data: AttrData::F32(vec![]),
        };
        assert_eq!(
            m.set_attribute("c", curve),
            Err(GeometryError::DomainNotOnMesh(Domain::CurvePoint))
        );
        assert!(matches!(
            m.set_attribute("", weights(8)),
            Err(GeometryError::BadAttributeName(_))
        ));
    }

    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "the positions are small whole numbers, exact in f32"
    )]
    fn transforming_moves_positions_and_keeps_faces_attributes_and_policy() {
        let mut m = cube();
        m.set_attribute(
            "weight",
            Attribute {
                domain: Domain::Point,
                data: AttrData::F32((0..8_u8).map(f32::from).collect()),
            },
        )
        .unwrap();
        m.set_triangulation_mode(crate::TriangulationMode::CurvedQuads);
        let moved = m
            .transformed(|[x, y, z]| [x * 2.0 + 10.0, y, z - 1.0])
            .unwrap();
        assert_eq!(
            moved.positions()[6],
            [12.0, 1.0, 0.0],
            "(1, 1, 1) is stretched and moved"
        );
        assert_eq!(moved.face_sizes(), m.face_sizes());
        assert_eq!(moved.corners(), m.corners());
        assert_eq!(moved.attributes(), m.attributes());
        assert_eq!(moved.triangulation_mode(), m.triangulation_mode());
        // The original is untouched.
        assert_eq!(m.positions()[6], [1.0, 1.0, 1.0]);
    }

    #[test]
    fn a_transform_that_makes_a_position_infinite_is_refused() {
        let error = cube().transformed(|[x, y, z]| [x / 0.0, y, z]).unwrap_err();
        assert!(matches!(error, GeometryError::InvalidMesh(_)), "{error:?}");
    }
}
