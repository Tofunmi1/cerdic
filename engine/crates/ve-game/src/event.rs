//! Inputs and step results. Rules can react to `grab`, `release`, and `use` events:
//! `grab` and `release` carry hand/item ids in `a`/`b`; `use` carries the hand id
//! and nearest in-reach entity (or no `b`). Inputs accept `follow=left_hand` and
//! `follow=right_hand`; kinds accept `grab=1`.
use crate::Effect;
use std::collections::BTreeMap;
/// Which tracked hand supplies an input pose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandSide {
    /// Left hand.
    Left,
    /// Right hand.
    Right,
}
/// One tracked hand sample for a fixed simulation step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandInput {
    /// Side of this tracked hand.
    pub side: HandSide,
    /// Hand position in world coordinates.
    pub position: [f64; 3],
    /// Whether the grip is held.
    pub grip: bool,
    /// Whether the trigger is held.
    pub trigger: bool,
}
/// A hand's world-space orientation for one fixed simulation step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandRotation {
    /// Side of this tracked hand.
    pub side: HandSide,
    /// Quaternion in x, y, z, w order.
    pub rotation: [f64; 4],
}
/// One fixed-step input sample.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Inputs {
    /// Tracked hand samples (missing sides are untracked).
    pub hands: Vec<HandInput>,
    /// Hand orientations; absent or untracked hands use identity.
    pub hand_rotations: Vec<HandRotation>,
    /// Viewer position in world coordinates, when supplied by the host.
    pub viewer: Option<[f64; 3]>,
    /// Named numeric axes/values.
    pub axes: BTreeMap<String, f64>,
    /// Pressed input names.
    pub pressed: Vec<String>,
    /// Host-raised named events for this step.
    pub host: Vec<HostEvent>,
}
/// A named event raised by a host with optional entity participants.
#[derive(Clone, Debug, PartialEq)]
pub struct HostEvent {
    /// Event name, without the `host:` prefix.
    pub name: String,
    /// Optional first participant id.
    pub a: Option<u32>,
    /// Optional second participant id.
    pub b: Option<u32>,
}
/// Terminal state of the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Status {
    /// Simulation continues.
    Playing,
    /// A win goal fired.
    Won,
    /// A loss goal fired.
    Lost,
}
/// Result of one deterministic step.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    /// Current status.
    pub status: Status,
    /// Deferred host effects.
    pub effects: Vec<Effect>,
    /// Events processed this step.
    pub events_count: usize,
}
