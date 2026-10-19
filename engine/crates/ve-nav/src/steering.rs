//! Pure steering maths, separated from crowd state so it is easy to test.

use glam::Vec2;

use crate::mesh::NavMesh;

/// How far apart neighbours push each other, as a multiple of the summed radius.
pub(crate) const SEPARATION_RANGE: f32 = 2.0;
/// How far ahead, in seconds, reciprocal avoidance looks for a collision.
pub(crate) const AVOID_HORIZON: f32 = 1.5;

/// Magnitude of the separation push between two agents `distance` apart.
///
/// Zero beyond `range`; grows smoothly as they touch, and is capped at the summed radius so a pair
/// of coincident agents cannot produce an unbounded velocity.
pub(crate) fn separation_push(distance: f32, radius_sum: f32, range: f32) -> f32 {
    if distance >= range || distance <= 1.0e-6 {
        return 0.0;
    }
    (1.0 - distance / range) * radius_sum
}

/// Magnitude of the reciprocal-avoidance push for two agents closing at `closing` metres/second.
///
/// This is the ORCA-lite rule: a linear look-ahead that fades to zero at [`AVOID_HORIZON`]. It is
/// applied half to each agent, which keeps the pair's relative steering reciprocal.
pub(crate) fn avoidance_push(distance: f32, closing: f32, horizon: f32) -> f32 {
    if closing <= 0.0 || distance <= 1.0e-6 {
        return 0.0;
    }
    let time_to_collision = distance / closing;
    if time_to_collision >= horizon {
        return 0.0;
    }
    (1.0 - time_to_collision / horizon) * closing
}

/// Moves `from -> to`, but never onto a blocked cell: if the full move is blocked, try sliding along
/// one axis, and otherwise stay put. This is how the crowd stays on the nav mesh.
pub(crate) fn clamp_to_walkable(nav: &NavMesh, from: Vec2, to: Vec2) -> Vec2 {
    if nav.is_walkable([to.x, 0.0, to.y]) {
        return to;
    }
    let slide_x = Vec2::new(to.x, from.y);
    if nav.is_walkable([slide_x.x, 0.0, slide_x.y]) {
        return slide_x;
    }
    let slide_z = Vec2::new(from.x, to.y);
    if nav.is_walkable([slide_z.x, 0.0, slide_z.y]) {
        return slide_z;
    }
    from
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separation_is_zero_outside_range_and_grows_as_agents_touch() {
        assert!(separation_push(2.0, 1.0, 2.0).abs() < f32::EPSILON);
        assert!(separation_push(3.0, 1.0, 2.0).abs() < f32::EPSILON);
        let touching = separation_push(0.5, 1.0, 2.0);
        let close = separation_push(0.1, 1.0, 2.0);
        assert!(touching > 0.0 && close > touching);
    }

    #[test]
    fn avoidance_only_fires_for_closing_agents_within_the_horizon() {
        assert!(avoidance_push(1.0, 0.0, 1.0).abs() < f32::EPSILON);
        assert!(avoidance_push(1.0, -1.0, 1.0).abs() < f32::EPSILON);
        assert!(avoidance_push(10.0, 1.0, 1.0).abs() < f32::EPSILON);
        assert!(avoidance_push(0.2, 2.0, 1.0) > 0.0);
    }
}
