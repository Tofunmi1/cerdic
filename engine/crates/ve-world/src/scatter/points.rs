//! The point set: positions plus named channels.
use super::ScatterError;
use std::collections::BTreeMap;

/// Points in the world with named per-point values (scalar `f32` or vector `[f32; 3]` channels).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PointSet {
    positions: Vec<[f32; 3]>,
    scalars: BTreeMap<String, Vec<f32>>,
    vectors: BTreeMap<String, Vec<[f32; 3]>>,
}

impl PointSet {
    /// A set of points with no channels.
    #[must_use]
    pub fn new(positions: Vec<[f32; 3]>) -> Self {
        Self {
            positions,
            scalars: BTreeMap::new(),
            vectors: BTreeMap::new(),
        }
    }

    /// Number of points.
    #[must_use]
    pub fn len(&self) -> usize {
        self.positions.len()
    }

    /// Whether there are no points.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// World positions.
    #[must_use]
    pub fn positions(&self) -> &[[f32; 3]] {
        &self.positions
    }

    /// Mutable world positions (the channels stay aligned: only the values move).
    pub fn positions_mut(&mut self) -> &mut [[f32; 3]] {
        &mut self.positions
    }

    /// A scalar channel, if present.
    #[must_use]
    pub fn channel(&self, name: &str) -> Option<&[f32]> {
        self.scalars.get(name).map(Vec::as_slice)
    }

    /// A vector channel, if present.
    #[must_use]
    pub fn vector_channel(&self, name: &str) -> Option<&[[f32; 3]]> {
        self.vectors.get(name).map(Vec::as_slice)
    }

    fn check(&self, name: &str, got: usize) -> Result<(), ScatterError> {
        if name.is_empty() {
            return Err(ScatterError::BadChannel { name: name.into() });
        }
        if got != self.positions.len() {
            return Err(ScatterError::LengthMismatch {
                name: name.into(),
                expected: self.positions.len(),
                got,
            });
        }
        Ok(())
    }

    /// Sets (or replaces) a scalar channel; one value per point.
    ///
    /// # Errors
    /// `BadChannel` for an empty name, `LengthMismatch` for a wrong length.
    pub fn set_channel(&mut self, name: &str, values: Vec<f32>) -> Result<(), ScatterError> {
        self.check(name, values.len())?;
        self.scalars.insert(name.into(), values);
        Ok(())
    }

    /// Sets (or replaces) a vector channel; one value per point.
    ///
    /// # Errors
    /// `BadChannel` for an empty name, `LengthMismatch` for a wrong length.
    pub fn set_vector_channel(
        &mut self,
        name: &str,
        values: Vec<[f32; 3]>,
    ) -> Result<(), ScatterError> {
        self.check(name, values.len())?;
        self.vectors.insert(name.into(), values);
        Ok(())
    }

    /// All channel names, scalar and vector, sorted.
    #[must_use]
    pub fn channel_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .scalars
            .keys()
            .chain(self.vectors.keys())
            .cloned()
            .collect();
        names.sort();
        names
    }

    /// Keeps the points whose entry in `keep` is true, and the same entries of every channel. A `keep` of the wrong length keeps nothing.
    pub fn retain(&mut self, keep: &[bool]) {
        fn pick<T: Clone>(items: &[T], keep: &[bool]) -> Vec<T> {
            items
                .iter()
                .zip(keep)
                .filter(|(_, k)| **k)
                .map(|(item, _)| item.clone())
                .collect()
        }
        if keep.len() != self.positions.len() {
            self.positions.clear();
            self.scalars.values_mut().for_each(Vec::clear);
            self.vectors.values_mut().for_each(Vec::clear);
            return;
        }
        self.positions = pick(&self.positions, keep);
        for values in self.scalars.values_mut() {
            *values = pick(values, keep);
        }
        for values in self.vectors.values_mut() {
            *values = pick(values, keep);
        }
    }

    /// Appends the points of `other`; both sets must have the same channels (an empty set takes the other's).
    ///
    /// # Errors
    /// `BadChannel` naming a channel only one side has.
    pub fn merge(&mut self, other: &Self) -> Result<(), ScatterError> {
        if self.is_empty() && self.scalars.is_empty() && self.vectors.is_empty() {
            *self = other.clone();
            return Ok(());
        }
        let mismatch = |a: &Self, b: &Self| {
            a.scalars
                .keys()
                .chain(a.vectors.keys())
                .find(|k| !b.scalars.contains_key(*k) && !b.vectors.contains_key(*k))
                .cloned()
        };
        if let Some(name) = mismatch(self, other).or_else(|| mismatch(other, self)) {
            return Err(ScatterError::BadChannel { name });
        }
        self.positions.extend_from_slice(&other.positions);
        for (name, values) in &mut self.scalars {
            values.extend_from_slice(&other.scalars[name]);
        }
        for (name, values) in &mut self.vectors {
            values.extend_from_slice(&other.vectors[name]);
        }
        Ok(())
    }
}
