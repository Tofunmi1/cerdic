//! Navigation for generated levels: a walkable grid built from triangles, A* plus string pulling,
//! and a small deterministic crowd.
//!
//! Everything here is pure Rust, so it runs on native and `wasm32`, and every result is
//! deterministic: no hash-map iteration order, no order-dependent float sums. Levels are generated
//! from our own geometry, so the nav mesh is generated from the same triangles the renderer draws;
//! nothing is imported.
//!
//! # Pipeline
//! [`NavMesh::build`] rasterises triangles into a heightfield of cells and keeps the surfaces an
//! agent can stand on (slope, headroom, step), then erodes them by the agent radius and stores the
//! result as a walkable grid. [`NavMesh::find_path`] runs A* over that grid and pulls the route
//! tight with the funnel algorithm. [`Crowd`] steers up to [`MAX_AGENTS`] agents along those paths,
//! and [`behaviour`] holds a few goal pickers (`wander`, `patrol`, `chase`, `flee`).
#![expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "grid dimensions and cell coordinates are small; f32 and usize round-trip exactly here"
)]

mod agent;
mod astar;
mod behaviour;
mod build;
mod config;
mod crowd;
mod error;
mod funnel;
mod grid;
mod mesh;
mod path;
mod raycast;
mod steering;

pub use agent::{Agent, AgentParams, AgentStatus};
pub use behaviour::{Chase, Flee, Goal, Patrol, Wander, line_of_sight};
pub use config::NavConfig;
pub use crowd::{Crowd, MAX_AGENTS};
pub use error::NavError;
pub use mesh::NavMesh;
pub use path::Path;

pub use build::MAX_CELLS;
