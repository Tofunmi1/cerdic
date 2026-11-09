//! Segment queries against the `solid` boxes of the world, for the host: a third-person camera arm shortens against walls with these.
use super::World;

impl World {
    /// The distance along the segment from `from` to `to` at which it first enters a `solid` box, or `None` when it reaches `to` clear.
    /// A start inside a box counts as a hit at distance 0.
    #[must_use]
    pub fn solid_hit(&self, from: [f64; 3], to: [f64; 3]) -> Option<f64> {
        let delta = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
        let length = delta.iter().map(|d| d * d).sum::<f64>().sqrt();
        if length < 1e-12 {
            return None;
        }
        let mut nearest: Option<f64> = None;
        for entity in &self.entities {
            let Some(extents) = self.def.kinds.iter().find(|kind| kind.name == entity.kind).and_then(|kind| kind.solid) else {
                continue;
            };
            let (mut enter, mut leave) = (0.0_f64, 1.0_f64);
            let mut inside = true;
            for axis in 0..3 {
                let (low, high) = (entity.position[axis] - extents[axis] / 2.0, entity.position[axis] + extents[axis] / 2.0);
                if delta[axis].abs() < 1e-12 {
                    inside &= from[axis] >= low && from[axis] <= high;
                    continue;
                }
                let (a, b) = ((low - from[axis]) / delta[axis], (high - from[axis]) / delta[axis]);
                enter = enter.max(a.min(b));
                leave = leave.min(a.max(b));
            }
            if inside && enter <= leave {
                let distance = enter * length;
                nearest = Some(nearest.map_or(distance, |best| best.min(distance)));
            }
        }
        nearest
    }
}
