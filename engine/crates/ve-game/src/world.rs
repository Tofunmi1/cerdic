//! Deterministic fixed-step simulation.
mod state;
pub(crate) use state::matches_selector;
pub use state::{Entity, Snapshot, World};
