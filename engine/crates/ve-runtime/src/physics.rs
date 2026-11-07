//! Rigid-body physics in play mode (`phys.body`, ADR 0035).
//!
//! When play mode starts, every part with a body goes into a [`ve_physics::World`] at its authored pose; each fixed
//! step (the same count as behaviours) the world advances and the parts are posed from it. A part picked up becomes
//! kinematic and follows the hand; let go, it is dynamic again and keeps the hand's speed (a throw). Leaving play mode
//! drops the world and puts every part back.
//!
//! A part is posed as `Transform = (rotation, position - rotation * centre)`, so it turns about its own centre even
//! though its vertices are authored in world coordinates.

include!("physics/part_1.rs");
include!("physics/part_2.rs");
include!("physics/part_3.rs");
include!("physics/part_4.rs");
include!("physics/part_5.rs");
