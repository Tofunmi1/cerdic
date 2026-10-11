//! The vocabulary of interaction: who points (pointers), what they point at (targets), what they do
//! (frames) and what happens (events). Mouse, touch, controllers, hands and gaze all become the same
//! [`PointerFrame`], so everything downstream is written once.

use glam::Vec3;

use crate::geometry::Ray;
use crate::pose::{Placement3, Pose};

/// Identifies one pointing device, stable for as long as it exists (for example 0 for the mouse, 1
/// and 2 for the left and right controllers).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PointerId(pub u32);

/// Identifies something that can be pointed at; the host maps it to its own object (for example an
/// entity). Meaningless to this crate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TargetId(pub u64);

/// What one pointer is doing this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerFrame {
    /// Which pointer this is.
    pub id: PointerId,
    /// The ray it points along, if it is tracked and pointing.
    pub ray: Option<Ray>,
    /// The pose of the hand or controller holding it, if it has one. A held object then follows
    /// this pose rigidly, turning with the hand. Without one (a mouse, gaze) a held object only
    /// follows the point where the ray meets it.
    pub grip: Option<Pose>,
    /// The primary button (mouse button, trigger, pinch) is down.
    pub select: bool,
    /// The secondary button (squeeze, grip) is down.
    pub squeeze: bool,
    /// Metres to move a held object along the ray this frame (mouse wheel, thumbstick).
    pub reach: f32,
    /// Whether pressing the primary button on a target grabs it (a mouse drag) instead of only
    /// selecting it. Controllers grab with the squeeze button instead.
    pub select_grabs: bool,
}

impl PointerFrame {
    /// A frame for a pointer that is present but idle and pointing nowhere.
    #[must_use]
    pub fn idle(id: PointerId) -> Self {
        Self {
            id,
            ray: None,
            grip: None,
            select: false,
            squeeze: false,
            reach: 0.0,
            select_grabs: false,
        }
    }
}

/// A ray hit on a target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// What was hit.
    pub target: TargetId,
    /// Distance along the ray.
    pub distance: f32,
    /// The point that was hit.
    pub point: Vec3,
}

/// What the host provides: the objects in the world.
pub trait World {
    /// The closest target hit by `ray` within `max_distance`.
    fn pick(&self, ray: &Ray, max_distance: f32) -> Option<Hit>;

    /// The current placement of a target, or `None` if it no longer exists.
    fn placement(&self, target: TargetId) -> Option<Placement3>;

    /// Whether the target may be picked up. Targets are grabbable by default.
    fn grabbable(&self, _target: TargetId) -> bool {
        true
    }

    /// The socket-snap hook: where a released target should settle instead of where it was let go,
    /// if anywhere. Called once as each grab ends. The snapping rules live in the host.
    fn snap(&self, _target: TargetId, _released: Placement3) -> Option<Placement3> {
        None
    }
}

/// Something that happened this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// A pointer began pointing at a target.
    HoverStart {
        /// The pointer.
        pointer: PointerId,
        /// The target it now points at.
        target: TargetId,
    },
    /// A pointer stopped pointing at a target (it moved away, vanished, or the target did).
    HoverEnd {
        /// The pointer.
        pointer: PointerId,
        /// The target it no longer points at.
        target: TargetId,
    },
    /// The primary button went down on a target.
    SelectStart {
        /// The pointer.
        pointer: PointerId,
        /// The target under it.
        target: TargetId,
    },
    /// The primary button went up after going down on a target.
    SelectEnd {
        /// The pointer.
        pointer: PointerId,
        /// The target it went down on.
        target: TargetId,
        /// It came up on the same target without dragging it: a click.
        click: bool,
    },
    /// A pointer picked a target up.
    GrabStart {
        /// The pointer that grabbed (the first holder when two hands hold it).
        pointer: PointerId,
        /// The target.
        target: TargetId,
    },
    /// The last hand let go of a target.
    GrabEnd {
        /// The pointer that let go last.
        pointer: PointerId,
        /// The target.
        target: TargetId,
        /// Where it was when released.
        placement: Placement3,
        /// Where the host's snap hook wants it to settle, if anywhere.
        snapped: Option<Placement3>,
    },
}

/// The result of one interaction update.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Output {
    /// What happened, in order.
    pub events: Vec<Event>,
    /// Where every currently held target should be now. The host applies these.
    pub placements: Vec<(TargetId, Placement3)>,
}
