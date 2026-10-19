//! Interaction logic with no engine code.
//!
//! Every way of pointing (mouse, touch, controllers, hands, gaze) becomes a [`PointerFrame`]; the
//! [`Interaction`] state machine turns those into hover, select and grab [`Event`]s and moves held
//! objects, including two-handed scale and rotate. The host implements [`World`] (picking, object
//! placements, the socket-snap hook) and applies the placements the machine returns. Nothing here
//! depends on the engine, the GPU or the browser, so all of it is tested natively.

mod geometry;
mod gesture;
mod gizmo;
mod grab;
mod held;
mod machine;
mod pointer;
mod pose;

pub use geometry::{Ray, Shape, ray_affine_box};
pub use gesture::{PinchConfig, PinchRecognizer, PinchState, gaze_pointer, hand_pointer};
pub use gizmo::{
    CameraView, Constraint as GizmoConstraint, Delta as GizmoDelta, GizmoDrag, Mode as GizmoMode,
    Snap as GizmoSnap,
};
pub use grab::{Anchor, SCALE_LIMITS, follow_one, follow_two};
pub use machine::Interaction;
pub use pointer::{Event, Hit, Output, PointerFrame, PointerId, TargetId, World};
pub use pose::{Placement3, Pose, rotations_equivalent};
