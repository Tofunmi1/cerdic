//! Engine-agnostic core.
//!
//! Everything extensible in the engine (geometry nodes, components, tools, packs) registers
//! under a [`Name`] in a [`Registry`]. This crate must never depend on Bevy or on platform code;
//! `cargo xtask check` enforces that.

mod name;
mod registry;

pub use name::{MAX_NAME_LEN, Name, NameError};
pub use registry::{Registry, RegistryError};
