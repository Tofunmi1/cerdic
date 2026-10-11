//! Engine-agnostic core.
//!
//! Everything extensible in the engine (geometry nodes, components, tools, packs) registers
//! under a [`Name`] in a [`Registry`]. This crate must never depend on Bevy or on platform code;
//! `cargo xtask check` enforces that.

mod frame_stats;
mod governor;
mod name;
mod registry;

pub use frame_stats::{DROPPED_FRAME_FACTOR, FrameStats, FrameSummary};
pub use governor::{Governor, GovernorConfig};
pub use name::{MAX_NAME_LEN, Name, NameError};
pub use registry::{Registry, RegistryError};
