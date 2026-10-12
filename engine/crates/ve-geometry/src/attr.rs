//! Attributes: named values attached to the elements of a geometry, grouped by domain.

/// Which elements an attribute has one value for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Domain {
    /// One value per vertex.
    Point,
    /// One value per undirected edge.
    Edge,
    /// One value per face.
    Face,
    /// One value per face corner (a vertex as seen from one face), for UVs and shading normals.
    Corner,
    /// One value per point of a curve.
    CurvePoint,
    /// One value per instance.
    Instance,
}

/// The values of an attribute, one scalar per element.
#[derive(Clone, Debug, PartialEq)]
pub enum AttrData {
    /// 32-bit floats.
    F32(Vec<f32>),
    /// 64-bit floats.
    F64(Vec<f64>),
    /// Signed integers.
    I32(Vec<i32>),
    /// Unsigned integers (ids, labels, material slots).
    U32(Vec<u32>),
}

impl AttrData {
    /// Number of values.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::F32(v) => v.len(),
            Self::F64(v) => v.len(),
            Self::I32(v) => v.len(),
            Self::U32(v) => v.len(),
        }
    }

    /// Whether there are no values.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A named set of values on one domain.
#[derive(Clone, Debug, PartialEq)]
pub struct Attribute {
    /// Which elements the values belong to.
    pub domain: Domain,
    /// The values.
    pub data: AttrData,
}

/// What an operator does to an attribute it did not create. Operators declare one per attribute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Propagation {
    /// Carried over unchanged (element identity is kept).
    Preserve,
    /// Blended where elements are created or merged; only for continuous values such as weights.
    Interpolate,
    /// Copied from a source element, never averaged; for ids, labels and material slots.
    Discrete,
    /// Produced by the operator itself.
    Create,
    /// Dropped because it can no longer be correct.
    Invalidate,
}

impl Propagation {
    /// Whether numeric blending is allowed. Ids and labels must never be averaged.
    #[must_use]
    pub fn may_blend(self) -> bool {
        self == Self::Interpolate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_follow_the_data() {
        assert_eq!(AttrData::F32(vec![0.0; 3]).len(), 3);
        assert!(AttrData::U32(Vec::new()).is_empty());
    }

    #[test]
    fn only_interpolate_may_blend() {
        for p in [
            Propagation::Preserve,
            Propagation::Discrete,
            Propagation::Create,
            Propagation::Invalidate,
        ] {
            assert!(!p.may_blend());
        }
        assert!(Propagation::Interpolate.may_blend());
    }
}
