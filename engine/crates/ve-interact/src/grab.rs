//! How held objects move: the maths of one-handed and two-handed manipulation.
//!
//! Both are expressed as a change from a *baseline*: the object's placement when the current set of
//! holders last changed, and where each holder was then. With no hand motion the result is exactly
//! the baseline, so grabbing (or a second hand joining, or one letting go) never makes an object
//! jump.

use glam::{Quat, Vec3};

use crate::pose::Placement3;

/// Smallest and largest absolute scale a two-handed pinch can give an object.
pub const SCALE_LIMITS: (f32, f32) = (0.05, 20.0);

/// Hands closer together than this (metres) when a two-handed grab starts cannot define a scale.
const MIN_BASE_SPAN: f32 = 1e-3;

/// A holder's tracked point, and its orientation when it has one (a controller does, a mouse ray
/// does not).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    /// Where the holder is.
    pub position: Vec3,
    /// How it is turned, if known.
    pub rotation: Option<Quat>,
}

/// The object's placement when one hand holds it and that hand has moved from `base` to `now`.
///
/// With orientations on both anchors the object turns with the hand, orbiting it. Without them the
/// object only translates by the hand's movement.
#[must_use]
pub fn follow_one(baseline: &Placement3, base: Anchor, now: Anchor) -> Placement3 {
    match (base.rotation, now.rotation) {
        (Some(from), Some(to)) => {
            let turn = (to * from.inverse()).normalize();
            Placement3 {
                translation: now.position + turn * (baseline.translation - base.position),
                rotation: (turn * baseline.rotation).normalize(),
                scale: baseline.scale,
            }
        }
        _ => Placement3 {
            translation: baseline.translation + (now.position - base.position),
            ..*baseline
        },
    }
}

/// The object's placement when two hands hold it and have moved from `base` to `now`.
///
/// The object moves by the same amount as the midpoint between the hands, and scales and turns
/// about its own centre: it scales with the distance between the hands and turns by the rotation
/// that carries the old hand-to-hand direction onto the new one. (Rolling both hands about the line
/// between them does not turn the object.) Using the midpoint's motion rather than orbiting the
/// midpoint keeps a distant object, held by rays, from flying away as the hands spread.
#[must_use]
pub fn follow_two(baseline: &Placement3, base: [Anchor; 2], now: [Anchor; 2]) -> Placement3 {
    let base_span = base[1].position - base[0].position;
    let span = now[1].position - now[0].position;
    let (base_length, length) = (base_span.length(), span.length());
    if base_length < MIN_BASE_SPAN || length < f32::EPSILON {
        // Hands on top of each other say nothing about scale or direction; follow the first.
        return follow_one(baseline, base[0], now[0]);
    }
    let wanted = baseline.scale * (length / base_length);
    let scale = wanted.clamp(SCALE_LIMITS.0, SCALE_LIMITS.1);
    let turn = Quat::from_rotation_arc(base_span / base_length, span / length);
    let base_middle = (base[0].position + base[1].position) * 0.5;
    let middle = (now[0].position + now[1].position) * 0.5;
    Placement3 {
        translation: baseline.translation + (middle - base_middle),
        rotation: (turn * baseline.rotation).normalize(),
        scale,
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::FRAC_PI_2;

    use proptest::prelude::*;

    use super::*;

    fn point(x: f32, y: f32, z: f32) -> Anchor {
        Anchor {
            position: Vec3::new(x, y, z),
            rotation: None,
        }
    }

    fn held_at(translation: Vec3) -> Placement3 {
        Placement3 {
            translation,
            rotation: Quat::from_rotation_z(0.3),
            scale: 1.5,
        }
    }

    #[test]
    fn no_hand_motion_leaves_the_object_exactly_where_it_was() {
        let baseline = held_at(Vec3::new(1.0, 2.0, 3.0));
        let hand = Anchor {
            position: Vec3::new(0.5, 1.0, 2.0),
            rotation: Some(Quat::from_rotation_y(0.7)),
        };
        assert!(follow_one(&baseline, hand, hand).approx_eq(&baseline, 1e-6));
        let other = point(1.0, 1.0, 1.0);
        assert!(follow_two(&baseline, [hand, other], [hand, other]).approx_eq(&baseline, 1e-6));
    }

    #[test]
    fn a_hand_without_orientation_only_translates_the_object() {
        let baseline = held_at(Vec3::new(1.0, 0.0, 0.0));
        let moved = follow_one(&baseline, point(0.0, 0.0, 0.0), point(0.0, 2.0, -1.0));
        assert!(
            moved
                .translation
                .abs_diff_eq(Vec3::new(1.0, 2.0, -1.0), 1e-6)
        );
        assert!(moved.rotation.abs_diff_eq(baseline.rotation, 1e-6));
    }

    #[test]
    fn a_turning_hand_carries_the_object_around_it_rigidly() {
        // The object sits one metre in front of the hand along +x; the hand turns a quarter about y.
        let baseline = Placement3 {
            translation: Vec3::new(1.0, 0.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: 1.0,
        };
        let base = Anchor {
            position: Vec3::ZERO,
            rotation: Some(Quat::IDENTITY),
        };
        let now = Anchor {
            position: Vec3::ZERO,
            rotation: Some(Quat::from_rotation_y(FRAC_PI_2)),
        };
        let moved = follow_one(&baseline, base, now);
        // A quarter turn about +y takes +x to -z.
        assert!(
            moved
                .translation
                .abs_diff_eq(Vec3::new(0.0, 0.0, -1.0), 1e-5),
            "{}",
            moved.translation
        );
        assert!(
            moved
                .rotation
                .abs_diff_eq(Quat::from_rotation_y(FRAC_PI_2), 1e-5)
        );
    }

    #[test]
    fn spreading_the_hands_scales_the_object_about_their_midpoint() {
        let baseline = Placement3 {
            translation: Vec3::new(0.0, 1.0, 0.0),
            rotation: Quat::IDENTITY,
            scale: 1.0,
        };
        let base = [point(-0.2, 1.0, 0.0), point(0.2, 1.0, 0.0)];
        let now = [point(-0.4, 1.0, 0.0), point(0.4, 1.0, 0.0)];
        let result = follow_two(&baseline, base, now);
        assert!((result.scale - 2.0).abs() < 1e-5);
        assert!(
            result.translation.abs_diff_eq(baseline.translation, 1e-5),
            "midpoint did not move"
        );
    }

    #[test]
    fn a_distant_object_scales_in_place_and_follows_the_midpoint_motion() {
        let baseline = Placement3 {
            translation: Vec3::new(1.0, 0.0, -8.0),
            rotation: Quat::IDENTITY,
            scale: 1.0,
        };
        let base = [point(-0.5, 0.0, 0.0), point(0.5, 0.0, 0.0)];
        // Spread about the same midpoint: the object stays exactly where it is, twice as big.
        let spread = [point(-1.0, 0.0, 0.0), point(1.0, 0.0, 0.0)];
        let result = follow_two(&baseline, base, spread);
        assert!(result.translation.abs_diff_eq(baseline.translation, 1e-5));
        assert!((result.scale - 2.0).abs() < 1e-5);
        // Raise both hands as well: the object rises by the same amount.
        let raised = [point(-1.0, 0.5, 0.0), point(1.0, 0.5, 0.0)];
        let result = follow_two(&baseline, base, raised);
        assert!(
            result
                .translation
                .abs_diff_eq(Vec3::new(1.0, 0.5, -8.0), 1e-5)
        );
    }

    #[test]
    fn turning_the_pair_a_quarter_turns_the_object_a_quarter() {
        let baseline = held_at(Vec3::ZERO);
        let base = [point(-1.0, 0.0, 0.0), point(1.0, 0.0, 0.0)];
        // The hand-to-hand line swings from +x to -z: a quarter turn about +y.
        let now = [point(0.0, 0.0, 1.0), point(0.0, 0.0, -1.0)];
        let result = follow_two(&baseline, base, now);
        let expected = (Quat::from_rotation_y(FRAC_PI_2) * baseline.rotation).normalize();
        assert!(result.rotation.abs_diff_eq(expected, 1e-5));
        assert!((result.scale - baseline.scale).abs() < 1e-5);
    }

    #[test]
    fn scale_is_limited() {
        let baseline = held_at(Vec3::ZERO);
        let base = [point(-0.01, 0.0, 0.0), point(0.01, 0.0, 0.0)];
        let huge = [point(-50.0, 0.0, 0.0), point(50.0, 0.0, 0.0)];
        assert!((follow_two(&baseline, base, huge).scale - SCALE_LIMITS.1).abs() < 1e-4);
        let tiny = [point(0.0, 0.0, 0.0), point(1e-5, 0.0, 0.0)];
        assert!((follow_two(&baseline, base, tiny).scale - SCALE_LIMITS.0).abs() < 1e-4);
    }

    #[test]
    fn hands_starting_on_top_of_each_other_fall_back_to_one_hand() {
        let baseline = held_at(Vec3::new(1.0, 0.0, 0.0));
        let base = [point(0.0, 0.0, 0.0), point(0.0, 0.0, 0.0)];
        let now = [point(0.0, 1.0, 0.0), point(0.0, 3.0, 0.0)];
        let result = follow_two(&baseline, base, now);
        assert!(
            result
                .translation
                .abs_diff_eq(Vec3::new(1.0, 1.0, 0.0), 1e-5)
        );
        assert!((result.scale - baseline.scale).abs() < 1e-6);
    }

    proptest! {
        #[test]
        fn moving_both_hands_together_moves_the_object_the_same_amount(
            bx in -2.0_f32..2.0, by in -2.0_f32..2.0, bz in -2.0_f32..2.0,
            dx in -3.0_f32..3.0, dy in -3.0_f32..3.0, dz in -3.0_f32..3.0,
        ) {
            let baseline = held_at(Vec3::new(bx, by, bz));
            let a = point(-0.3, 1.0, 0.1);
            let b = point(0.4, 1.2, -0.2);
            let shift = Vec3::new(dx, dy, dz);
            let moved = [
                Anchor { position: a.position + shift, ..a },
                Anchor { position: b.position + shift, ..b },
            ];
            let result = follow_two(&baseline, [a, b], moved);
            prop_assert!(result.translation.abs_diff_eq(baseline.translation + shift, 1e-4));
            prop_assert!(result.rotation.abs_diff_eq(baseline.rotation, 1e-4));
            prop_assert!((result.scale - baseline.scale).abs() < 1e-4);
        }

        #[test]
        fn results_are_always_finite_and_in_limits(
            coords in prop::collection::vec(-100.0_f32..100.0, 12),
            scale in 0.05_f32..20.0,
        ) {
            let baseline = Placement3 { scale, ..held_at(Vec3::new(coords[0], coords[1], coords[2])) };
            let anchor = |i: usize| point(coords[i], coords[i + 1], coords[i + 2]);
            let result = follow_two(&baseline, [anchor(3), anchor(6)], [anchor(9), anchor(0)]);
            prop_assert!(result.is_finite());
            prop_assert!(result.scale >= SCALE_LIMITS.0 - 1e-6 && result.scale <= SCALE_LIMITS.1 + 1e-6);
        }
    }
}
