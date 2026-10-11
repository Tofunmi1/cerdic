//! Hand gestures: pinch detection from fingertip positions, and pointers built from hands and gaze.

use glam::Vec3;

use crate::geometry::Ray;
use crate::pointer::{PointerFrame, PointerId};
use crate::pose::Pose;

/// Thresholds for [`PinchRecognizer`], in metres between thumb tip and index tip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PinchConfig {
    /// A pinch begins when the tips come this close.
    pub start_distance: f32,
    /// A pinch ends when the tips part this far (larger than `start_distance`, so a pinch held at
    /// the threshold does not flicker).
    pub end_distance: f32,
}

impl Default for PinchConfig {
    fn default() -> Self {
        Self {
            start_distance: 0.02,
            end_distance: 0.035,
        }
    }
}

/// The result of a pinch update.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PinchState {
    /// The hand is pinching.
    pub pinching: bool,
    /// How closed the pinch is, 0 (open) to 1 (tips touching), for animation.
    pub strength: f32,
}

/// Decides, frame by frame, whether a hand is pinching.
#[derive(Clone, Copy, Debug)]
pub struct PinchRecognizer {
    config: PinchConfig,
    pinching: bool,
}

impl PinchRecognizer {
    /// A recognizer with the given thresholds. If they are out of order the end distance is raised
    /// to the start distance, so the hysteresis band is never negative.
    #[must_use]
    pub fn new(config: PinchConfig) -> Self {
        Self {
            config: PinchConfig {
                start_distance: config.start_distance.max(0.0),
                end_distance: config.end_distance.max(config.start_distance),
            },
            pinching: false,
        }
    }

    /// Feeds the thumb-tip and index-tip positions of this frame. Positions that are not finite
    /// (tracking lost) end the pinch.
    pub fn update(&mut self, thumb_tip: Vec3, index_tip: Vec3) -> PinchState {
        if !thumb_tip.is_finite() || !index_tip.is_finite() {
            return self.lost();
        }
        let distance = thumb_tip.distance(index_tip);
        if self.pinching {
            if distance >= self.config.end_distance {
                self.pinching = false;
            }
        } else if distance <= self.config.start_distance {
            self.pinching = true;
        }
        let band = (self.config.end_distance - self.config.start_distance).max(f32::EPSILON);
        PinchState {
            pinching: self.pinching,
            strength: ((self.config.end_distance - distance) / band).clamp(0.0, 1.0),
        }
    }

    /// The hand is no longer tracked: any pinch ends.
    pub fn lost(&mut self) -> PinchState {
        self.pinching = false;
        PinchState {
            pinching: false,
            strength: 0.0,
        }
    }
}

/// A pointer for a tracked hand: it points along `aim`, a pinch is its primary button and a held
/// object follows the hand (`wrist`) rigidly, turning with it.
#[must_use]
pub fn hand_pointer(
    id: PointerId,
    aim: Option<Ray>,
    wrist: Option<Pose>,
    pinch: PinchState,
) -> PointerFrame {
    PointerFrame {
        id,
        ray: aim,
        grip: wrist,
        select: pinch.pinching,
        squeeze: false,
        reach: 0.0,
        select_grabs: true,
    }
}

/// A pointer for gaze-and-pinch: the eyes (or head) aim, a pinch anywhere in view is the button.
/// A held object follows the point on the gaze ray, since the hand's position is not the aim.
#[must_use]
pub fn gaze_pointer(id: PointerId, gaze: Option<Ray>, pinch: PinchState) -> PointerFrame {
    PointerFrame {
        id,
        ray: gaze,
        grip: None,
        select: pinch.pinching,
        squeeze: false,
        reach: 0.0,
        select_grabs: true,
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn tips(distance: f32) -> (Vec3, Vec3) {
        (Vec3::ZERO, Vec3::new(distance, 0.0, 0.0))
    }

    #[test]
    fn a_pinch_starts_close_and_ends_only_when_the_fingers_part_well() {
        let mut recognizer = PinchRecognizer::new(PinchConfig::default());
        let state = |r: &mut PinchRecognizer, d: f32| {
            let (a, b) = tips(d);
            r.update(a, b)
        };
        assert!(!state(&mut recognizer, 0.05).pinching);
        assert!(
            !state(&mut recognizer, 0.025).pinching,
            "between thresholds starts nothing"
        );
        assert!(state(&mut recognizer, 0.015).pinching);
        assert!(
            state(&mut recognizer, 0.03).pinching,
            "the pinch holds inside the band"
        );
        assert!(!state(&mut recognizer, 0.04).pinching);
        assert!(
            !state(&mut recognizer, 0.025).pinching,
            "and does not restart inside the band"
        );
    }

    #[test]
    fn strength_runs_from_open_to_touching() {
        let mut recognizer = PinchRecognizer::new(PinchConfig::default());
        let strength = |r: &mut PinchRecognizer, d: f32| {
            let (a, b) = tips(d);
            r.update(a, b).strength
        };
        assert!(strength(&mut recognizer, 0.1).abs() < 1e-6);
        assert!((strength(&mut recognizer, 0.0) - 1.0).abs() < 1e-6);
        let middle = strength(&mut recognizer, 0.0275);
        assert!((middle - 0.5).abs() < 1e-4, "{middle}");
    }

    #[test]
    fn lost_tracking_ends_the_pinch() {
        let mut recognizer = PinchRecognizer::new(PinchConfig::default());
        let (a, b) = tips(0.01);
        assert!(recognizer.update(a, b).pinching);
        assert!(!recognizer.update(Vec3::NAN, b).pinching);
        assert!(!recognizer.update(a, Vec3::INFINITY).pinching);
        // It takes a fresh close approach to pinch again.
        assert!(recognizer.update(a, b).pinching);
        assert!(!recognizer.lost().pinching);
    }

    #[test]
    fn disordered_thresholds_are_repaired() {
        let mut recognizer = PinchRecognizer::new(PinchConfig {
            start_distance: 0.03,
            end_distance: 0.01,
        });
        let (a, b) = tips(0.02);
        // 0.02 m is inside the repaired band's start distance, so it pinches.
        assert!(recognizer.update(a, b).pinching);
    }

    #[test]
    fn hand_and_gaze_pointers_press_with_the_pinch() {
        let closed = PinchState {
            pinching: true,
            strength: 1.0,
        };
        let ray = Ray::new(Vec3::ZERO, Vec3::NEG_Z);
        let hand = hand_pointer(
            PointerId(3),
            ray,
            Some(Pose {
                position: Vec3::X,
                rotation: glam::Quat::IDENTITY,
            }),
            closed,
        );
        assert!(hand.select && hand.select_grabs && hand.grip.is_some());
        let gaze = gaze_pointer(PointerId(4), ray, closed);
        assert!(gaze.select && gaze.grip.is_none() && gaze.ray == ray);
    }

    proptest! {
        #[test]
        fn pinching_never_flickers_while_the_distance_stays_inside_the_band(
            distances in prop::collection::vec(0.021_f32..0.034, 1..60),
            start_closed in any::<bool>(),
        ) {
            let mut recognizer = PinchRecognizer::new(PinchConfig::default());
            let (a, b) = tips(if start_closed { 0.01 } else { 0.05 });
            let initial = recognizer.update(a, b).pinching;
            for d in distances {
                let (a, b) = tips(d);
                prop_assert_eq!(recognizer.update(a, b).pinching, initial);
            }
        }
    }
}
