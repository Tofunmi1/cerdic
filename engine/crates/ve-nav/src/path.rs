//! A finished route through the level.

/// A route from a start point to an end point, as corners in world space.
///
/// Corners are ordered from start to end. The funnel pass removes the redundant points of the raw
/// grid path, so a path that turns once has three corners rather than one per cell.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    points: Vec<[f32; 3]>,
}

impl Path {
    /// Wraps the given corners.
    pub(crate) fn new(points: Vec<[f32; 3]>) -> Self {
        Self { points }
    }

    /// The corners, from start to end.
    #[must_use]
    pub fn points(&self) -> &[[f32; 3]] {
        &self.points
    }

    /// How many corners the path has.
    #[must_use]
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Whether the path has no corners at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Total length in metres, walking from corner to corner.
    #[must_use]
    pub fn length(&self) -> f32 {
        self.points
            .windows(2)
            .map(|w| {
                let d = [w[1][0] - w[0][0], w[1][1] - w[0][1], w[1][2] - w[0][2]];
                (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
            })
            .sum()
    }
}
