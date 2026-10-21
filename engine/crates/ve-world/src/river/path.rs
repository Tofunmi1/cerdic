//! A world-space polyline for a river or a road.

/// A polyline in world x, z metres. The carvers resample it as a centripetal Catmull-Rom spline at half-cell spacing.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Path {
    /// The points, in order of flow (rivers) or travel (roads).
    pub points: Vec<[f32; 2]>,
}
