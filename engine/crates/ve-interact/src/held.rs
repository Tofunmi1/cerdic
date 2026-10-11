//! A held target and the hands holding it.

use crate::grab::{Anchor, follow_one, follow_two};
use crate::pointer::{PointerFrame, PointerId};
use crate::pose::Placement3;

/// One hand holding a target, and where it was when the hold (or the set of holds) began.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Holder {
    pub(crate) pointer: PointerId,
    pub(crate) base: Anchor,
}

/// A held target.
#[derive(Debug)]
pub(crate) struct Held {
    pub(crate) holders: Vec<Holder>,
    /// The target's placement when the set of holders last changed.
    pub(crate) baseline: Placement3,
    /// Where it was when first picked up.
    pub(crate) started: Placement3,
    /// Where it is now.
    pub(crate) current: Placement3,
}

impl Held {
    /// A target just picked up by one hand.
    pub(crate) fn new(pointer: PointerId, base: Anchor, placement: Placement3) -> Self {
        Self {
            holders: vec![Holder { pointer, base }],
            baseline: placement,
            started: placement,
            current: placement,
        }
    }

    /// How far (metres) the target has been carried since it was picked up.
    pub(crate) fn carried(&self) -> f32 {
        self.current.translation.distance(self.started.translation)
    }

    /// Moves the target to follow its holders, given where each holder is now (`None` for a holder
    /// that lost tracking this frame, in which case the target stays where it is).
    pub(crate) fn advance(&mut self, now: &[Option<Anchor>]) {
        let next = match (self.holders.as_slice(), now) {
            ([one], [Some(now)]) => Some(follow_one(&self.baseline, one.base, *now)),
            ([a, b], [Some(now_a), Some(now_b)]) => Some(follow_two(
                &self.baseline,
                [a.base, b.base],
                [*now_a, *now_b],
            )),
            _ => None,
        };
        if let Some(placement) = next.filter(Placement3::is_finite) {
            self.current = placement;
        }
    }
}

/// The anchor a pointer offers this frame: its grip pose if it has one, else the point on its ray
/// at the grab `distance`.
pub(crate) fn anchor_of(frame: &PointerFrame, distance: f32) -> Option<Anchor> {
    if let Some(grip) = frame.grip {
        return Some(Anchor {
            position: grip.position,
            rotation: Some(grip.rotation),
        });
    }
    frame.ray.map(|ray| Anchor {
        position: ray.at(distance),
        rotation: None,
    })
}
